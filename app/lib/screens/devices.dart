import 'dart:async';
import 'dart:typed_data';

import 'package:flutter/material.dart';

import '../app_scope.dart';
import '../l10n/gen/app_localizations.dart';
import '../src/rust/api.dart';
import '../widgets/help.dart';
import '../widgets/qr.dart';
import '../widgets/scan_or_paste.dart';
import 'send_flow.dart';

/// Our devices: the main one first. On the main device, add one (in person) or
/// remove one.
class DevicesScreen extends StatelessWidget {
  const DevicesScreen({super.key});

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final state = AppScope.of(context);
    final devices = state.node.ownDevices();
    final main = state.node.isMainDevice();
    return Scaffold(
      appBar: AppBar(
        title: Text(l.devices),
        actions: [HelpAction(title: l.devices, text: l.devicesHelp)],
      ),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          for (final d in devices)
            ListTile(
              contentPadding: EdgeInsets.zero,
              leading: Icon(d.main ? Icons.smartphone : Icons.tablet_android),
              title: Text(
                d.thisDevice
                    ? l.thisDevice
                    : (d.main ? l.mainDevice : l.otherDevice),
              ),
              subtitle: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    [
                      if (d.thisDevice && d.main) l.mainDevice,
                      // A short id tells two other devices apart.
                      if (!d.main) d.id.substring(0, 8).toUpperCase(),
                    ].join(' · '),
                  ),
                  if (!d.joined)
                    Row(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Icon(
                          Icons.error_outline,
                          size: 16,
                          color: Theme.of(context).colorScheme.error,
                        ),
                        const SizedBox(width: 6),
                        Expanded(child: Text(l.deviceWaiting)),
                      ],
                    ),
                ],
              ),
              trailing: main && !d.main
                  ? TextButton(
                      onPressed: () => _remove(context, d),
                      child: Text(l.removeDevice),
                    )
                  : null,
            ),
          if (main && devices.length < 4) ...[
            const SizedBox(height: 16),
            FilledButton.icon(
              icon: const Icon(Icons.add_to_home_screen),
              label: Text(l.addDevice),
              onPressed: () => Navigator.of(context).push(
                MaterialPageRoute<void>(
                  builder: (_) => const AddDeviceScreen(),
                ),
              ),
            ),
          ],
        ],
      ),
    );
  }

  Future<void> _remove(BuildContext context, DeviceItem device) async {
    final l = AppLocalizations.of(context);
    final state = AppScope.read(context);
    final ok = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(l.removeDeviceTitle),
        content: Text(
          device.joined ? l.removeDeviceBody : l.removeWaitingDeviceBody,
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context, false),
            child: Text(MaterialLocalizations.of(context).cancelButtonLabel),
          ),
          TextButton(
            onPressed: () => Navigator.pop(context, true),
            child: Text(l.removeDevice),
          ),
        ],
      ),
    );
    if (ok != true) return;
    try {
      await state.node.retireDevice(id: device.id);
      state.changed();
    } on Object catch (e) {
      if (context.mounted) showSnack(context, reasonText(context, e));
    }
  }
}

/// Main device: scan the new device's request in person, confirm, then show it the setup as an
/// animated QR code.
class AddDeviceScreen extends StatefulWidget {
  const AddDeviceScreen({super.key});

  @override
  State<AddDeviceScreen> createState() => _AddDeviceScreenState();
}

class _AddDeviceScreenState extends State<AddDeviceScreen> {
  bool _busy = false;

  Future<void> _onRequest(Uint8List request) async {
    if (_busy) return;
    _busy = true;
    try {
      final l = AppLocalizations.of(context);
      final state = AppScope.read(context);
      final ok = await showDialog<bool>(
        context: context,
        builder: (context) => AlertDialog(
          title: Text(l.addDeviceConfirmTitle),
          content: Text(l.addDeviceConfirmBody),
          actions: [
            TextButton(
              onPressed: () => Navigator.pop(context, false),
              child: Text(MaterialLocalizations.of(context).cancelButtonLabel),
            ),
            TextButton(
              onPressed: () => Navigator.pop(context, true),
              child: Text(l.addDevice),
            ),
          ],
        ),
      );
      if (ok != true || !mounted) return;
      final QrAnimation? animation;
      try {
        animation = await state.node.approveDevice(request: request);
      } on Object catch (e) {
        if (mounted) showSnack(context, reasonText(context, e));
        return;
      }
      state.changed();
      if (!mounted) return;
      if (animation == null) {
        showSnack(context, l.reason_too_large_for_channel);
        return;
      }
      await Navigator.of(context).pushReplacement(
        MaterialPageRoute<void>(
          fullscreenDialog: true,
          builder: (_) => AnimatedQrScreen(
            animation: animation!,
            title: l.addDeviceShowSetup,
            hint: l.addDeviceSetupHint,
            // A one-time transfer to a camera we know nothing about: the easiest code first.
            pace: QrPace.easy,
          ),
        ),
      );
    } finally {
      _busy = false;
    }
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return Scaffold(
      appBar: AppBar(
        title: Text(l.addDevice),
        actions: [HelpAction(title: l.addDevice, text: l.addDeviceHelp)],
      ),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          SizedBox(
            height: 360,
            child: CameraScanner(
              onScanned: (_) async {},
              onDeviceRequest: _onRequest,
            ),
          ),
        ],
      ),
    );
  }
}

/// A new installation becomes another device of the user's MEM: show the request, then read
/// the setup the main device shows.
class JoinDeviceScreen extends StatefulWidget {
  const JoinDeviceScreen({super.key});

  @override
  State<JoinDeviceScreen> createState() => _JoinDeviceScreenState();
}

class _JoinDeviceScreenState extends State<JoinDeviceScreen> {
  String? _request;
  Object? _error;
  bool _done = false;

  /// A setup arrived that this request cannot open (made for an earlier one); stays until the
  /// right one arrives.
  bool _wrongSetup = false;

  @override
  void initState() {
    super.initState();
    unawaited(_start());
  }

  Future<void> _start() async {
    try {
      final text = await AppScope.read(context).node.startDeviceJoin();
      if (mounted) setState(() => _request = text);
    } on Object catch (e) {
      if (mounted) setState(() => _error = e);
    }
  }

  @override
  void dispose() {
    if (!_done) AppScope.read(context).node.cancelDeviceJoin();
    super.dispose();
  }

  Future<void> _onSetup(Uint8List setup) async {
    final l = AppLocalizations.of(context);
    final state = AppScope.read(context);
    try {
      await state.node.finishDeviceJoin(setup: setup);
    } on Object catch (e) {
      if (!mounted) return;
      if (e == 'not_for_this_device') {
        setState(() => _wrongSetup = true);
      } else {
        showSnack(context, reasonText(context, e));
      }
      return;
    }
    _done = true;
    state.changed();
    if (!mounted) return;
    showSnack(context, l.joinDone);
    Navigator.of(context).popUntil((route) => route.isFirst);
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final request = _request;
    final qr = request == null ? null : qrForText(text: request);
    return Scaffold(
      appBar: AppBar(
        title: Text(l.joinTitle),
        actions: [HelpAction(title: l.joinTitle, text: l.joinHelp)],
      ),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          if (_error != null)
            Text(reasonText(context, _error!))
          else ...[
            if (qr != null) Center(child: QrMatrix(qr: qr, size: 260)),
            const SizedBox(height: 16),
            if (_wrongSetup) ...[
              Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Icon(
                    Icons.error_outline,
                    size: 16,
                    color: Theme.of(context).colorScheme.error,
                  ),
                  const SizedBox(width: 6),
                  Expanded(child: Text(l.joinWrongSetup)),
                ],
              ),
              const SizedBox(height: 16),
            ],
            SizedBox(
              height: 320,
              child: CameraScanner(
                onScanned: (_) async {},
                onDeviceSetup: _onSetup,
              ),
            ),
          ],
        ],
      ),
    );
  }
}

/// Tells the user that the main device removed this device, then replaces its data with a
/// fresh identity.
Future<void> showDeviceRetired(BuildContext context) async {
  final l = AppLocalizations.of(context);
  final state = AppScope.read(context);
  await showDialog<void>(
    context: context,
    barrierDismissible: false,
    builder: (context) => AlertDialog(
      title: Text(l.deviceRetiredTitle),
      content: Text(l.deviceRetiredBody),
      actions: [
        TextButton(
          onPressed: () => Navigator.pop(context),
          child: Text(MaterialLocalizations.of(context).okButtonLabel),
        ),
      ],
    ),
  );
  await state.node.forgetThisDevice();
  state.changed();
}
