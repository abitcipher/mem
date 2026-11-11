import 'dart:math' as math;

import 'package:flutter/material.dart';

import '../app_scope.dart';
import '../l10n/gen/app_localizations.dart';
import '../src/rust/api.dart';
import '../widgets/help.dart';
import '../widgets/scan_or_paste.dart';
import 'channels.dart';
import '../widgets/qr.dart';
import 'sound_pair.dart';

/// Nearby: how many MEM phones this one reaches on the local network, and two ways to
/// find another phone when the network hides them from each other: by sound, or by code.
class NearbyScreen extends StatelessWidget {
  const NearbyScreen({super.key});

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final state = AppScope.of(context);
    final lanOn = state.node.lanEnabled();
    final devices = state.node.nearbyDevices();
    void push(Widget screen) =>
        Navigator.of(context)
            .push(MaterialPageRoute<void>(builder: (_) => screen));
    return Scaffold(
      appBar: AppBar(
        title: Text(l.nearbyTitle),
        actions: [HelpAction(title: l.nearbyTitle, text: l.nearbyHelp)],
      ),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          ListTile(
            contentPadding: EdgeInsets.zero,
            leading: Icon(
              !lanOn
                  ? Icons.wifi_off
                  : devices > 0
                  ? Icons.wifi
                  : Icons.wifi_find,
            ),
            title: Text(lanOn ? l.nearbyDevices(devices) : l.lanOffShort),
            trailing: TextButton(
              onPressed: () => push(const ChannelsScreen()),
              child: Text(l.channels),
            ),
          ),
          const SizedBox(height: 16),
          FilledButton.icon(
            icon: const Icon(Icons.graphic_eq),
            label: Text(l.connectBySound),
            onPressed: lanOn
                ? () => push(const SoundPairScreen(mode: SoundMode.connect))
                : null,
          ),
          const SizedBox(height: 12),
          OutlinedButton.icon(
            icon: const Icon(Icons.qr_code_2),
            label: Text(l.connectByCode),
            onPressed: lanOn ? () => push(const ConnectCodeScreen()) : null,
          ),
          if (!lanOn) ...[
            const SizedBox(height: 12),
            Text(l.reason_no_local_network),
          ],
        ],
      ),
    );
  }
}

/// Connect by code: this phone's connect hint as one QR code, and the camera to read
/// the other phone's. Either direction is enough.
class ConnectCodeScreen extends StatefulWidget {
  const ConnectCodeScreen({super.key});

  @override
  State<ConnectCodeScreen> createState() => _ConnectCodeScreenState();
}

class _ConnectCodeScreenState extends State<ConnectCodeScreen> {
  QrAnimation? _code;
  Object? _error;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) => _load());
  }

  Future<void> _load() async {
    try {
      final code = await AppScope.read(context).node.connectHintQr();
      if (mounted) setState(() => _code = code);
    } on Object catch (e) {
      if (mounted) setState(() => _error = e);
    }
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final code = _code;
    final frame = code?.frame(n: 0);
    return ShowingCode(
      child: Scaffold(
        appBar: AppBar(
          title: Text(l.connectByCode),
          actions: [HelpAction(title: l.connectByCode, text: l.nearbyHelp)],
        ),
        body: ListView(
          padding: const EdgeInsets.all(16),
          children: [
            if (_error case final e?)
              Text(reasonText(context, e))
            else ...[
              Text(l.connectCodeHint, textAlign: TextAlign.center),
              const SizedBox(height: 12),
              if (frame != null)
                LayoutBuilder(
                  // As large as the width allows: the bigger the modules, the farther a camera
                  // reads it.
                  builder: (context, box) => Center(
                    child: QrMatrix(
                      qr: frame,
                      // The camera below stays on screen.
                      size: math.min(
                        box.maxWidth - 16,
                        MediaQuery.sizeOf(context).height * 0.45,
                      ),
                    ),
                  ),
                ),
              const SizedBox(height: 16),
              CameraScanner(onScanned: (_) async {}),
            ],
          ],
        ),
      ),
    );
  }
}
