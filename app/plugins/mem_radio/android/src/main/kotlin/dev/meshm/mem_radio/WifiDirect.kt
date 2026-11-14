package dev.meshm.mem_radio

import android.annotation.SuppressLint
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.net.wifi.WpsInfo
import android.net.wifi.p2p.WifiP2pConfig
import android.net.wifi.p2p.WifiP2pDevice
import android.net.wifi.p2p.WifiP2pInfo
import android.net.wifi.p2p.WifiP2pManager
import android.net.wifi.p2p.nsd.WifiP2pDnsSdServiceInfo
import android.net.wifi.p2p.nsd.WifiP2pDnsSdServiceRequest
import android.os.Build
import android.os.Handler
import android.os.Looper
import java.security.SecureRandom

/**
 * Wi-Fi Direct between nearby phones. Moves no MEM data itself: it finds other MEM
 * phones, forms a P2P group, and reports the group owner's address and Iroh endpoint so the Rust
 * side can run the usual encounter over the link.
 *
 * - Advertisement: DNS-SD service `_mem0._udp` with TXT `v` (version), `n` (random nonce,
 *   rotated every 15 minutes), `i`/`p` (Iroh endpoint id and port: a per-start transport key,
 *   never the identity), `q` (1 if this phone can host a group others join silently, Android
 *   10+), and, while hosting such a group, `g`/`s` (network name and passphrase).
 * - Roles: the smaller nonce hosts. Both Android 10+: the host creates a group with its own name
 *   and passphrase, the other joins it with them: no dialog. Otherwise the other phone uses the
 *   classic connect and Android shows its "Invitation to connect" dialog on the host.
 * - Everything read from a record is untrusted and length-checked; a bad record is ignored.
 */
@SuppressLint("MissingPermission") // The plugin checks permissions before start().
class WifiDirect(private val context: Context, private val events: Events) {
    interface Events {
        /** Joined a group as a client: dial the owner's endpoint. */
        fun peer(id: String, ip: String, port: Int)

        /** "running", "stopped", "unsupported", "error". */
        fun wifiState(state: String)
    }

    private val main = Handler(Looper.getMainLooper())
    private val random = SecureRandom()
    private var manager: WifiP2pManager? = null
    private var channel: WifiP2pManager.Channel? = null
    private var receiver: BroadcastReceiver? = null
    private var running = false
    private var endpointId = ""
    private var port = 0
    private var nonce = ""
    private var nonceSince = 0L
    private var hosting: Pair<String, String>? = null

    /** Records seen, by device address (for dialling the owner after joining). */
    private val records = HashMap<String, Map<String, String>>()

    /** The device we are joining, if any. */
    private var joining: String? = null
    private var joinStarted = 0L

    val isRunning: Boolean get() = running

    fun start(endpointId: String, port: Int) {
        if (running) return
        val m = context.getSystemService(Context.WIFI_P2P_SERVICE) as? WifiP2pManager
        if (m == null) {
            events.wifiState("unsupported")
            return
        }
        this.endpointId = endpointId
        this.port = port
        manager = m
        channel = m.initialize(context, Looper.getMainLooper(), null)
        running = true
        register()
        rotateNonce()
        advertise()
        listen()
        main.post(tick)
        events.wifiState("running")
    }

    fun stop() {
        if (!running) return
        running = false
        main.removeCallbacks(tick)
        val m = manager
        val c = channel
        if (m != null && c != null) {
            m.clearLocalServices(c, null)
            m.clearServiceRequests(c, null)
            m.stopPeerDiscovery(c, null)
            m.removeGroup(c, null)
        }
        receiver?.let { runCatching { context.unregisterReceiver(it) } }
        receiver = null
        hosting = null
        joining = null
        records.clear()
        events.wifiState("stopped")
    }

    /** Ends the current group (the encounter is over). Discovery goes on. */
    fun leaveGroup() {
        val m = manager ?: return
        val c = channel ?: return
        m.removeGroup(c, null)
        joining = null
        if (hosting != null) {
            hosting = null
            advertise()
        }
    }

    // Discovery stops on its own after a few minutes; renew it, and rotate the nonce.
    private val tick = object : Runnable {
        override fun run() {
            if (!running) return
            if (System.currentTimeMillis() - nonceSince > NONCE_EVERY_MS) {
                rotateNonce()
                advertise()
            }
            if (joining != null && System.currentTimeMillis() - joinStarted > JOIN_TIMEOUT_MS) {
                joining = null
            }
            discover()
            main.postDelayed(this, DISCOVER_EVERY_MS)
        }
    }

    private fun rotateNonce() {
        val b = ByteArray(4)
        random.nextBytes(b)
        nonce = b.joinToString("") { "%02x".format(it) }
        nonceSince = System.currentTimeMillis()
    }

    private fun canHostSilently() = Build.VERSION.SDK_INT >= 29

    private fun advertise() {
        val m = manager ?: return
        val c = channel ?: return
        val txt = HashMap<String, String>().apply {
            put("v", "1")
            put("n", nonce)
            put("i", endpointId)
            put("p", port.toString())
            put("q", if (canHostSilently()) "1" else "0")
            hosting?.let { (g, s) ->
                put("g", g)
                put("s", s)
            }
        }
        val info = WifiP2pDnsSdServiceInfo.newInstance(INSTANCE, SERVICE_TYPE, txt)
        m.clearLocalServices(c, object : WifiP2pManager.ActionListener {
            override fun onSuccess() = m.addLocalService(c, info, null)
            override fun onFailure(reason: Int) = m.addLocalService(c, info, null)
        })
    }

    private fun listen() {
        val m = manager ?: return
        val c = channel ?: return
        m.setDnsSdResponseListeners(
            c,
            { _, _, _ -> },
            { fullDomain, record, device -> onRecord(fullDomain, record, device) },
        )
        // Ask for our instance by name: a request for the service type alone is answered with
        // the instance names only, never with the TXT record that carries the details.
        m.addServiceRequest(
            c,
            WifiP2pDnsSdServiceRequest.newInstance(INSTANCE, SERVICE_TYPE),
            null,
        )
        discover()
    }

    private fun discover() {
        val m = manager ?: return
        val c = channel ?: return
        m.discoverServices(c, null)
    }

    private fun onRecord(fullDomain: String?, record: Map<String, String>?, device: WifiP2pDevice?) {
        if (!running || device == null || record == null) return
        if (fullDomain?.contains(SERVICE_TYPE.substringBefore('.')) != true) return
        val r = record.filter { (k, v) -> k.length <= 2 && v.length <= MAX_VALUE }
        val theirNonce = r["n"]?.takeIf { it.length == 8 } ?: return
        if (r["v"] != "1" || r["i"]?.length != 64 || r["p"]?.toIntOrNull() == null) return
        records[device.deviceAddress] = r
        if (joining != null || theirNonce == nonce) return
        val bothSilent = canHostSilently() && r["q"] == "1"
        if (nonce < theirNonce) {
            // We host. With silent groups we create ours and advertise how to join; otherwise
            // the other phone connects and Android asks us.
            if (bothSilent && hosting == null) host()
            return
        }
        val m = manager ?: return
        val c = channel ?: return
        val config = if (bothSilent) {
            val g = r["g"] ?: return // Not hosting yet: wait for the next record.
            val s = r["s"] ?: return
            if (Build.VERSION.SDK_INT < 29) return
            WifiP2pConfig.Builder().setNetworkName(g).setPassphrase(s).build()
        } else {
            WifiP2pConfig().apply {
                deviceAddress = device.deviceAddress
                wps.setup = WpsInfo.PBC
                groupOwnerIntent = 0
            }
        }
        joining = device.deviceAddress
        joinStarted = System.currentTimeMillis()
        m.connect(c, config, object : WifiP2pManager.ActionListener {
            override fun onSuccess() {}
            override fun onFailure(reason: Int) {
                joining = null
            }
        })
    }

    private fun host() {
        if (Build.VERSION.SDK_INT < 29) return
        val m = manager ?: return
        val c = channel ?: return
        val name = "DIRECT-" + randomText(2) + "-MEM" + randomText(4)
        val pass = randomText(16)
        val config = WifiP2pConfig.Builder().setNetworkName(name).setPassphrase(pass).build()
        m.createGroup(c, config, object : WifiP2pManager.ActionListener {
            override fun onSuccess() {
                hosting = name to pass
                advertise()
            }

            override fun onFailure(reason: Int) {}
        })
    }

    private fun randomText(n: Int): String {
        val chars = "abcdefghijkmnpqrstuvwxyzACDEFGHJKLMNPQRTUVWXY3479"
        return (1..n).map { chars[random.nextInt(chars.length)] }.joinToString("")
    }

    private fun register() {
        val r = object : BroadcastReceiver() {
            override fun onReceive(ctx: Context, intent: Intent) {
                if (intent.action == WifiP2pManager.WIFI_P2P_CONNECTION_CHANGED_ACTION) {
                    val m = manager ?: return
                    val c = channel ?: return
                    m.requestConnectionInfo(c) { info -> onConnection(info) }
                }
            }
        }
        val filter = IntentFilter(WifiP2pManager.WIFI_P2P_CONNECTION_CHANGED_ACTION)
        if (Build.VERSION.SDK_INT >= 33) {
            context.registerReceiver(r, filter, Context.RECEIVER_NOT_EXPORTED)
        } else {
            context.registerReceiver(r, filter)
        }
        receiver = r
    }

    // As a client, dial the owner's endpoint at its address. The owner only accepts.
    private fun onConnection(info: WifiP2pInfo?) {
        if (!running || info == null || !info.groupFormed || info.isGroupOwner) return
        val ip = info.groupOwnerAddress?.hostAddress ?: return
        val owner = joining?.let { records[it] } ?: records.values.firstOrNull { it["g"] != null }
            ?: return
        val id = owner["i"] ?: return
        val port = owner["p"]?.toIntOrNull()?.takeIf { it in 1..65535 } ?: return
        events.peer(id, ip, port)
    }

    companion object {
        const val INSTANCE = "mem"
        const val SERVICE_TYPE = "_mem0._udp"
        const val NONCE_EVERY_MS = 15 * 60 * 1000L
        const val DISCOVER_EVERY_MS = 30_000L
        const val JOIN_TIMEOUT_MS = 60_000L
        const val MAX_VALUE = 64
    }
}
