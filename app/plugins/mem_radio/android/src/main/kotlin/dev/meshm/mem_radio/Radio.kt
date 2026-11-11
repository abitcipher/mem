package dev.meshm.mem_radio

import android.annotation.SuppressLint
import android.bluetooth.BluetoothAdapter
import android.bluetooth.BluetoothDevice
import android.bluetooth.BluetoothGatt
import android.bluetooth.BluetoothGattCallback
import android.bluetooth.BluetoothGattCharacteristic
import android.bluetooth.BluetoothGattDescriptor
import android.bluetooth.BluetoothGattServer
import android.bluetooth.BluetoothGattServerCallback
import android.bluetooth.BluetoothGattService
import android.bluetooth.BluetoothManager
import android.bluetooth.BluetoothProfile
import android.bluetooth.BluetoothServerSocket
import android.bluetooth.le.AdvertiseCallback
import android.bluetooth.le.AdvertiseData
import android.bluetooth.le.AdvertiseSettings
import android.bluetooth.le.ScanCallback
import android.bluetooth.le.ScanFilter
import android.bluetooth.le.ScanResult
import android.bluetooth.le.ScanSettings
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.os.Build
import android.os.Handler
import android.os.HandlerThread
import android.os.ParcelUuid
import android.os.SystemClock
import java.io.IOException

/**
 * The BLE radio: advertises and scans at the same time, connects to peers with a
 * larger link nonce, and runs one byte stream per connection over L2CAP (Android 10+) or GATT.
 *
 * All state lives on one worker thread; Android's callbacks post to it. Permissions are checked
 * by the plugin before [start]; a call without them fails quietly into the "stopped" state.
 */
@SuppressLint("MissingPermission")
internal class Radio(private val context: Context, private val events: Events) : LinkHost {
    /** What the radio tells the app. Called on the worker thread. */
    interface Events {
        fun linkUp(link: Long)
        fun bytes(link: Long, data: ByteArray)
        fun linkDown(link: Long)
        fun sent(op: Long, ok: Boolean)
        fun state(state: String)
    }

    private val thread = HandlerThread("mem-radio").apply { start() }
    override val worker = Handler(thread.looper)

    private val manager = context.getSystemService(Context.BLUETOOTH_SERVICE) as? BluetoothManager
    private val adapter: BluetoothAdapter? get() = manager?.adapter

    /** The app wants the radio on (it stays wanted while Bluetooth itself is switched off). */
    private var wanted = false
    private var running = false
    private var lowPower = false

    private var nonce = Wire.newNonce()
    private var advertising = false
    private var scanning = false

    private var nextLink = 1L
    private val links = HashMap<Long, Link>()
    private val byAddress = HashMap<String, Link>()
    /** Central connections not yet carrying a stream. */
    private val connecting = HashMap<String, BluetoothGatt>()
    private val clientMtu = HashMap<String, Int>()
    private val serverMtu = HashMap<String, Int>()
    /** No new connection to an address until then (elapsed ms): after a link ends or fails. */
    private val cooldown = HashMap<String, Long>()

    private var server: BluetoothGattServer? = null
    private var l2capServer: BluetoothServerSocket? = null
    private var psm = 0

    fun start(lowPower: Boolean) = worker.post {
        wanted = true
        this.lowPower = lowPower
        startRadio()
    }

    fun stop() = worker.post {
        wanted = false
        stopRadio()
        events.state(STOPPED)
    }

    /** Foreground: fastest discovery. Background ("Stay reachable"): low power. */
    fun setLowPower(on: Boolean) = worker.post {
        if (lowPower == on) return@post
        lowPower = on
        if (running) {
            stopScan()
            startScan()
            stopAdvertising()
            startAdvertising()
        }
    }

    fun send(link: Long, op: Long, bytes: ByteArray) = worker.post {
        val l = links[link]
        if (l == null) events.sent(op, false) else l.send(op, bytes)
    }

    fun close(link: Long) = worker.post { links[link]?.let { lost(it) } }

    val isRunning: Boolean get() = running

    // ---- LinkHost ----

    override fun received(link: Link, bytes: ByteArray) {
        if (links[link.id] === link) events.bytes(link.id, bytes)
    }

    override fun sent(op: Long, ok: Boolean) = events.sent(op, ok)

    override fun lost(link: Link) {
        if (links.remove(link.id) == null) return
        if (byAddress[link.address] === link) byAddress.remove(link.address)
        link.close()
        cooldown[link.address] = SystemClock.elapsedRealtime() + COOLDOWN_MS
        events.linkDown(link.id)
    }

    private fun up(link: Link) {
        links[link.id] = link
        byAddress[link.address] = link
        events.linkUp(link.id)
    }

    // ---- start / stop ----

    private fun startRadio() {
        if (running) return
        val adapter = adapter
        if (adapter == null) {
            events.state(UNSUPPORTED)
            return
        }
        if (!adapter.isEnabled) {
            events.state(BLUETOOTH_OFF)
            return
        }
        try {
            openServer()
            openL2cap()
            running = true
            startScan()
            // Advertising starts once the GATT service is registered (onServiceAdded).
            worker.postDelayed(rotate, Wire.NONCE_ROTATE_MS)
            events.state(RUNNING)
        } catch (e: SecurityException) {
            stopRadio()
            events.state(NO_PERMISSION)
        }
    }

    private fun stopRadio() {
        worker.removeCallbacks(rotate)
        try {
            stopScan()
            stopAdvertising()
        } catch (_: SecurityException) {
        }
        for (l in links.values.toList()) lost(l)
        for (g in connecting.values) {
            try {
                g.disconnect()
                g.close()
            } catch (_: SecurityException) {
            }
        }
        connecting.clear()
        clientMtu.clear()
        serverMtu.clear()
        try {
            server?.close()
        } catch (_: SecurityException) {
        }
        server = null
        try {
            l2capServer?.close()
        } catch (_: IOException) {
        }
        l2capServer = null
        psm = 0
        running = false
    }

    /** Bluetooth switched off or on in the system settings. */
    private val adapterState = object : BroadcastReceiver() {
        override fun onReceive(c: Context, intent: Intent) {
            val state = intent.getIntExtra(BluetoothAdapter.EXTRA_STATE, BluetoothAdapter.ERROR)
            worker.post {
                when (state) {
                    BluetoothAdapter.STATE_TURNING_OFF, BluetoothAdapter.STATE_OFF -> if (running) {
                        stopRadio()
                        events.state(BLUETOOTH_OFF)
                    }
                    BluetoothAdapter.STATE_ON -> if (wanted) startRadio()
                }
            }
        }
    }

    init {
        context.registerReceiver(adapterState, IntentFilter(BluetoothAdapter.ACTION_STATE_CHANGED))
    }

    // ---- advertising (peripheral) ----

    private val rotate = object : Runnable {
        override fun run() {
            rotateNonce()
            worker.postDelayed(this, Wire.NONCE_ROTATE_MS)
        }
    }

    /** A new link nonce: restarts advertising. */
    private fun rotateNonce() {
        nonce = Wire.newNonce()
        if (advertising) {
            stopAdvertising()
            startAdvertising()
        }
    }

    private val advertiseCallback = object : AdvertiseCallback() {
        override fun onStartFailure(errorCode: Int) {
            // Scanning and connecting still work: this phone is then a central only.
            worker.post { advertising = false }
        }
    }

    private fun startAdvertising() {
        if (!running || advertising) return
        val advertiser = adapter?.bluetoothLeAdvertiser ?: return
        val ad = Wire.advert(nonce)
        val settings = AdvertiseSettings.Builder()
            .setAdvertiseMode(
                if (lowPower) AdvertiseSettings.ADVERTISE_MODE_LOW_POWER
                else AdvertiseSettings.ADVERTISE_MODE_LOW_LATENCY,
            )
            .setConnectable(true)
            .setTimeout(0)
            .setTxPowerLevel(AdvertiseSettings.ADVERTISE_TX_POWER_MEDIUM)
            .build()
        val data = AdvertiseData.Builder()
            .setIncludeDeviceName(ad.includesName)
            .setIncludeTxPowerLevel(ad.includesTxPower)
            .addServiceUuid(ParcelUuid(ad.serviceUuid))
            .build()
        val response = AdvertiseData.Builder()
            .setIncludeDeviceName(ad.includesName)
            .setIncludeTxPowerLevel(ad.includesTxPower)
            .addServiceData(ParcelUuid(ad.serviceData.first), ad.serviceData.second)
            .build()
        advertiser.startAdvertising(settings, data, response, advertiseCallback)
        advertising = true
    }

    private fun stopAdvertising() {
        if (!advertising) return
        advertising = false
        adapter?.bluetoothLeAdvertiser?.stopAdvertising(advertiseCallback)
    }

    // ---- GATT server (peripheral) ----

    private fun openServer() {
        val service = BluetoothGattService(Wire.SERVICE, BluetoothGattService.SERVICE_TYPE_PRIMARY)
        service.addCharacteristic(
            BluetoothGattCharacteristic(
                Wire.PSM,
                BluetoothGattCharacteristic.PROPERTY_READ,
                BluetoothGattCharacteristic.PERMISSION_READ,
            ),
        )
        service.addCharacteristic(
            BluetoothGattCharacteristic(
                Wire.RX,
                BluetoothGattCharacteristic.PROPERTY_WRITE,
                BluetoothGattCharacteristic.PERMISSION_WRITE,
            ),
        )
        val tx = BluetoothGattCharacteristic(
            Wire.TX,
            BluetoothGattCharacteristic.PROPERTY_NOTIFY,
            0,
        )
        tx.addDescriptor(
            BluetoothGattDescriptor(
                Wire.CCCD,
                BluetoothGattDescriptor.PERMISSION_READ or BluetoothGattDescriptor.PERMISSION_WRITE,
            ),
        )
        service.addCharacteristic(tx)
        server = manager?.openGattServer(context, serverCallback)?.also { it.addService(service) }
    }

    private fun serverTx(): BluetoothGattCharacteristic? =
        server?.getService(Wire.SERVICE)?.getCharacteristic(Wire.TX)

    private val serverCallback = object : BluetoothGattServerCallback() {
        override fun onServiceAdded(status: Int, service: BluetoothGattService) {
            worker.post { if (status == BluetoothGatt.GATT_SUCCESS) startAdvertising() }
        }

        override fun onConnectionStateChange(device: BluetoothDevice, status: Int, newState: Int) {
            if (newState != BluetoothProfile.STATE_DISCONNECTED) return
            val address = device.address
            worker.post {
                serverMtu.remove(address)
                (byAddress[address] as? GattServerLink)?.let { lost(it) }
            }
        }

        override fun onMtuChanged(device: BluetoothDevice, mtu: Int) {
            val address = device.address
            worker.post { serverMtu[address] = mtu }
        }

        override fun onCharacteristicReadRequest(
            device: BluetoothDevice,
            requestId: Int,
            offset: Int,
            characteristic: BluetoothGattCharacteristic,
        ) {
            val s = server ?: return
            if (characteristic.uuid != Wire.PSM) {
                s.sendResponse(device, requestId, BluetoothGatt.GATT_READ_NOT_PERMITTED, 0, null)
                return
            }
            val value = Wire.psmBytes(psm)
            if (offset > value.size) {
                s.sendResponse(device, requestId, BluetoothGatt.GATT_INVALID_OFFSET, offset, null)
            } else {
                s.sendResponse(
                    device,
                    requestId,
                    BluetoothGatt.GATT_SUCCESS,
                    offset,
                    value.copyOfRange(offset, value.size),
                )
            }
        }

        override fun onDescriptorWriteRequest(
            device: BluetoothDevice,
            requestId: Int,
            descriptor: BluetoothGattDescriptor,
            preparedWrite: Boolean,
            responseNeeded: Boolean,
            offset: Int,
            value: ByteArray?,
        ) {
            val ok = descriptor.uuid == Wire.CCCD &&
                descriptor.characteristic.uuid == Wire.TX &&
                !preparedWrite && offset == 0 && value != null && value.size == 2
            if (responseNeeded) {
                server?.sendResponse(
                    device,
                    requestId,
                    if (ok) BluetoothGatt.GATT_SUCCESS else BluetoothGatt.GATT_WRITE_NOT_PERMITTED,
                    0,
                    null,
                )
            }
            if (!ok) return
            val subscribe = value!!.contentEquals(BluetoothGattDescriptor.ENABLE_NOTIFICATION_VALUE)
            worker.post {
                val existing = byAddress[device.address]
                if (!subscribe) {
                    (existing as? GattServerLink)?.let { lost(it) }
                    return@post
                }
                val s = server ?: return@post
                val tx = serverTx() ?: return@post
                if (existing != null || links.size >= MAX_LINKS) {
                    s.cancelConnection(device)
                    return@post
                }
                up(
                    GattServerLink(
                        nextLink++,
                        device.address,
                        s,
                        device,
                        tx,
                        serverMtu[device.address] ?: DEFAULT_MTU,
                        this@Radio,
                    ),
                )
            }
        }

        override fun onCharacteristicWriteRequest(
            device: BluetoothDevice,
            requestId: Int,
            characteristic: BluetoothGattCharacteristic,
            preparedWrite: Boolean,
            responseNeeded: Boolean,
            offset: Int,
            value: ByteArray?,
        ) {
            // Chunks always fit one write: prepared (long) writes are never ours.
            val ok = characteristic.uuid == Wire.RX && !preparedWrite && offset == 0 && value != null
            if (responseNeeded) {
                server?.sendResponse(
                    device,
                    requestId,
                    if (ok) BluetoothGatt.GATT_SUCCESS else BluetoothGatt.GATT_WRITE_NOT_PERMITTED,
                    0,
                    null,
                )
            }
            if (!ok) return
            val piece = value!!.copyOf()
            val address = device.address
            worker.post {
                val link = byAddress[address] as? GattServerLink ?: return@post
                received(link, piece)
            }
        }

        override fun onNotificationSent(device: BluetoothDevice, status: Int) {
            val address = device.address
            worker.post {
                (byAddress[address] as? GattServerLink)?.notified(status == BluetoothGatt.GATT_SUCCESS)
            }
        }
    }

    // ---- L2CAP listener (peripheral, Android 10+) ----

    private fun openL2cap() {
        if (Build.VERSION.SDK_INT < 29) return
        val socket = try {
            adapter?.listenUsingInsecureL2capChannel()
        } catch (_: IOException) {
            null
        } ?: return
        l2capServer = socket
        psm = socket.psm
        Thread({
            while (true) {
                val client = try {
                    socket.accept()
                } catch (_: IOException) {
                    break
                }
                worker.post { accepted(socket, client) }
            }
        }, "mem-l2cap-accept").start()
    }

    private fun accepted(from: BluetoothServerSocket, client: android.bluetooth.BluetoothSocket) {
        val address = client.remoteDevice.address
        if (l2capServer !== from || byAddress.containsKey(address) || links.size >= MAX_LINKS) {
            try {
                client.close()
            } catch (_: IOException) {
            }
            return
        }
        val link = L2capLink(nextLink++, address, client, this, null)
        up(link)
        link.startThreads()
    }

    // ---- scanning and connecting (central) ----

    private val scanCallback = object : ScanCallback() {
        override fun onScanResult(callbackType: Int, result: ScanResult) {
            val data = result.scanRecord?.getServiceData(ParcelUuid(Wire.SERVICE))
            val theirs = Wire.nonceOf(data) ?: return
            val device = result.device
            worker.post { seen(device, theirs) }
        }

        override fun onBatchScanResults(results: MutableList<ScanResult>) {
            for (r in results) onScanResult(ScanSettings.CALLBACK_TYPE_ALL_MATCHES, r)
        }
    }

    private fun startScan() {
        if (!running || scanning) return
        val scanner = adapter?.bluetoothLeScanner ?: return
        val filters = listOf(ScanFilter.Builder().setServiceUuid(ParcelUuid(Wire.SERVICE)).build())
        val settings = ScanSettings.Builder()
            .setScanMode(
                if (lowPower) ScanSettings.SCAN_MODE_LOW_POWER
                else ScanSettings.SCAN_MODE_LOW_LATENCY,
            )
            .build()
        scanner.startScan(filters, settings, scanCallback)
        scanning = true
    }

    private fun stopScan() {
        if (!scanning) return
        scanning = false
        adapter?.bluetoothLeScanner?.stopScan(scanCallback)
    }

    private fun seen(device: BluetoothDevice, theirs: ByteArray) {
        if (!running) return
        val address = device.address
        if (byAddress.containsKey(address) || connecting.containsKey(address)) return
        val until = cooldown[address]
        if (until != null) {
            if (SystemClock.elapsedRealtime() < until) return
            cooldown.remove(address)
        }
        when (Wire.weConnect(nonce, theirs)) {
            // Same nonce: neither side would connect. Draw a new one.
            null -> rotateNonce()
            false -> Unit
            true -> if (links.size + connecting.size < MAX_LINKS) connect(device)
        }
    }

    private fun connect(device: BluetoothDevice) {
        val gatt = device.connectGatt(context, false, clientCallback, BluetoothDevice.TRANSPORT_LE)
            ?: return
        val address = device.address
        connecting[address] = gatt
        worker.postDelayed({
            if (connecting[address] === gatt) failConnect(address)
        }, CONNECT_TIMEOUT_MS)
    }

    private fun failConnect(address: String) {
        val gatt = connecting.remove(address) ?: return
        clientMtu.remove(address)
        gatt.disconnect()
        gatt.close()
        cooldown[address] = SystemClock.elapsedRealtime() + COOLDOWN_MS
    }

    private val clientCallback = object : BluetoothGattCallback() {
        override fun onConnectionStateChange(gatt: BluetoothGatt, status: Int, newState: Int) {
            val address = gatt.device.address
            worker.post {
                if (newState == BluetoothProfile.STATE_CONNECTED &&
                    status == BluetoothGatt.GATT_SUCCESS &&
                    connecting[address] === gatt
                ) {
                    gatt.requestConnectionPriority(BluetoothGatt.CONNECTION_PRIORITY_HIGH)
                    if (!gatt.requestMtu(MAX_MTU)) gatt.discoverServices()
                } else if (newState == BluetoothProfile.STATE_DISCONNECTED) {
                    if (connecting[address] === gatt) failConnect(address)
                    (byAddress[address] as? GattClientLink)?.let { lost(it) }
                }
            }
        }

        override fun onMtuChanged(gatt: BluetoothGatt, mtu: Int, status: Int) {
            val address = gatt.device.address
            worker.post {
                if (connecting[address] !== gatt) return@post
                clientMtu[address] = if (status == BluetoothGatt.GATT_SUCCESS) mtu else DEFAULT_MTU
                if (!gatt.discoverServices()) failConnect(address)
            }
        }

        override fun onServicesDiscovered(gatt: BluetoothGatt, status: Int) {
            val address = gatt.device.address
            worker.post {
                if (connecting[address] !== gatt) return@post
                val psmChar = gatt.getService(Wire.SERVICE)?.getCharacteristic(Wire.PSM)
                if (status != BluetoothGatt.GATT_SUCCESS || psmChar == null ||
                    !gatt.readCharacteristic(psmChar)
                ) {
                    failConnect(address)
                }
            }
        }

        @Deprecated("Android 13 adds the value parameter")
        override fun onCharacteristicRead(
            gatt: BluetoothGatt,
            characteristic: BluetoothGattCharacteristic,
            status: Int,
        ) {
            @Suppress("DEPRECATION")
            psmRead(gatt, characteristic, characteristic.value, status)
        }

        override fun onCharacteristicRead(
            gatt: BluetoothGatt,
            characteristic: BluetoothGattCharacteristic,
            value: ByteArray,
            status: Int,
        ) = psmRead(gatt, characteristic, value, status)

        override fun onDescriptorWrite(
            gatt: BluetoothGatt,
            descriptor: BluetoothGattDescriptor,
            status: Int,
        ) {
            val address = gatt.device.address
            worker.post {
                if (connecting[address] !== gatt) return@post
                val rx = gatt.getService(Wire.SERVICE)?.getCharacteristic(Wire.RX)
                if (status != BluetoothGatt.GATT_SUCCESS || rx == null) {
                    failConnect(address)
                    return@post
                }
                connecting.remove(address)
                up(
                    GattClientLink(
                        nextLink++,
                        address,
                        gatt,
                        rx,
                        clientMtu.remove(address) ?: DEFAULT_MTU,
                        this@Radio,
                    ),
                )
            }
        }

        override fun onCharacteristicWrite(
            gatt: BluetoothGatt,
            characteristic: BluetoothGattCharacteristic,
            status: Int,
        ) {
            val address = gatt.device.address
            worker.post {
                (byAddress[address] as? GattClientLink)?.written(status == BluetoothGatt.GATT_SUCCESS)
            }
        }

        @Deprecated("Android 13 adds the value parameter")
        override fun onCharacteristicChanged(
            gatt: BluetoothGatt,
            characteristic: BluetoothGattCharacteristic,
        ) {
            @Suppress("DEPRECATION")
            notified(gatt, characteristic, characteristic.value)
        }

        override fun onCharacteristicChanged(
            gatt: BluetoothGatt,
            characteristic: BluetoothGattCharacteristic,
            value: ByteArray,
        ) = notified(gatt, characteristic, value)
    }

    private fun psmRead(
        gatt: BluetoothGatt,
        characteristic: BluetoothGattCharacteristic,
        value: ByteArray?,
        status: Int,
    ) {
        if (characteristic.uuid != Wire.PSM) return
        val address = gatt.device.address
        val peerPsm = if (status == BluetoothGatt.GATT_SUCCESS) Wire.psmOf(value) else 0
        worker.post {
            if (connecting[address] !== gatt) return@post
            if (peerPsm > 0 && Build.VERSION.SDK_INT >= 29) openL2capTo(gatt, peerPsm) else useGatt(gatt)
        }
    }

    /** Connects the L2CAP channel off the worker (it blocks); GATT if that fails. */
    private fun openL2capTo(gatt: BluetoothGatt, peerPsm: Int) {
        val address = gatt.device.address
        Thread({
            val socket = try {
                if (Build.VERSION.SDK_INT >= 29) {
                    gatt.device.createInsecureL2capChannel(peerPsm).also { it.connect() }
                } else {
                    null
                }
            } catch (_: IOException) {
                null
            }
            worker.post {
                if (connecting[address] !== gatt) {
                    try {
                        socket?.close()
                    } catch (_: IOException) {
                    }
                    return@post
                }
                if (socket == null) {
                    useGatt(gatt)
                    return@post
                }
                connecting.remove(address)
                clientMtu.remove(address)
                val link = L2capLink(nextLink++, address, socket, this@Radio, gatt)
                up(link)
                link.startThreads()
            }
        }, "mem-l2cap-connect").start()
    }

    /** GATT fallback: subscribe to TX; the link is up once the subscription is written. */
    private fun useGatt(gatt: BluetoothGatt) {
        val address = gatt.device.address
        val tx = gatt.getService(Wire.SERVICE)?.getCharacteristic(Wire.TX)
        val cccd = tx?.getDescriptor(Wire.CCCD)
        if (tx == null || cccd == null || !gatt.setCharacteristicNotification(tx, true)) {
            failConnect(address)
            return
        }
        val started = if (Build.VERSION.SDK_INT >= 33) {
            gatt.writeDescriptor(cccd, BluetoothGattDescriptor.ENABLE_NOTIFICATION_VALUE) ==
                BluetoothGatt.GATT_SUCCESS
        } else {
            @Suppress("DEPRECATION")
            cccd.value = BluetoothGattDescriptor.ENABLE_NOTIFICATION_VALUE
            @Suppress("DEPRECATION")
            gatt.writeDescriptor(cccd)
        }
        if (!started) failConnect(address)
    }

    private fun notified(gatt: BluetoothGatt, characteristic: BluetoothGattCharacteristic, value: ByteArray?) {
        if (characteristic.uuid != Wire.TX || value == null) return
        val piece = value.copyOf()
        val address = gatt.device.address
        worker.post {
            val link = byAddress[address] as? GattClientLink ?: return@post
            received(link, piece)
        }
    }

    companion object {
        const val RUNNING = "running"
        const val STOPPED = "stopped"
        const val BLUETOOTH_OFF = "bluetoothOff"
        const val UNSUPPORTED = "unsupported"
        const val NO_PERMISSION = "noPermission"

        /** Matches `cla-ble`'s `MAX_LINKS`. */
        const val MAX_LINKS = 8
        const val DEFAULT_MTU = 23
        const val MAX_MTU = 517
        const val CONNECT_TIMEOUT_MS = 20_000L
        const val COOLDOWN_MS = 30_000L
    }
}
