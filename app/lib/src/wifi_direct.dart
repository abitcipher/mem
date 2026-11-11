import 'dart:async';
import 'dart:io';

import 'package:flutter/widgets.dart';
import 'package:mem_radio/mem_radio.dart';

import 'rust/api.dart';

/// Connects Wi-Fi Direct to the node. The plugin finds other MEM phones and forms a
/// group; the node runs the usual encounter over the link to the address the plugin reports.
///
/// Runs while Wi-Fi Direct is on in MEM and the app is in the foreground, or in the background
/// with "Stay reachable" on. A group with no encounter for [idle] is ended, so the phone's normal
/// Wi-Fi is not held up longer than needed. Android only.
class WifiDirectBridge with WidgetsBindingObserver {
  WifiDirectBridge(this.node, {required this.reachable});

  final MemNode node;
  final bool Function() reachable;

  /// What the plugin last reported, for the Channels screen.
  final state = ValueNotifier<String>('stopped');

  static bool get available => Platform.isAndroid;
  static const idle = Duration(seconds: 30);

  bool _foreground =
      WidgetsBinding.instance.lifecycleState == AppLifecycleState.resumed;
  StreamSubscription<RadioEvent>? _events;
  Timer? _watch;
  DateTime? _joined;

  void attach() {
    if (!available) return;
    WidgetsBinding.instance.addObserver(this);
    _events = MemRadio.events.listen((e) {
      switch (e) {
        case WifiPeer(:final id, :final ip, :final port):
          _joined = DateTime.now();
          node.wifiPeer(id: id, ip: ip, port: port);
        case WifiStateChanged(state: final s):
          state.value = s;
        default:
      }
    });
    _watch = Timer.periodic(const Duration(seconds: 10), (_) => _leaveIfIdle());
    unawaited(sync());
  }

  void _leaveIfIdle() {
    final since = _joined;
    if (since == null || node.wifiConnections() > 0) {
      if (since != null) _joined = DateTime.now();
      return;
    }
    if (DateTime.now().difference(since) >= idle) {
      _joined = null;
      unawaited(MemRadio.wifiLeaveGroup());
    }
  }

  /// Starts or stops Wi-Fi Direct to match the setting, the permissions and the app state.
  Future<void> sync() async {
    if (!available) return;
    final local = node.wifiLocal();
    final on =
        node.wifiDirectEnabled() &&
        (_foreground || reachable()) &&
        local != null;
    if (!on) {
      await MemRadio.wifiStop();
      return;
    }
    final status = await MemRadio.wifiStatus();
    if (!status.supported) {
      state.value = 'unsupported';
      return;
    }
    if (!status.permissions) {
      state.value = 'noPermission';
      return;
    }
    if (!status.running) await MemRadio.wifiStart(local.id, local.port);
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    final foreground =
        state == AppLifecycleState.resumed ||
        state == AppLifecycleState.inactive;
    if (foreground == _foreground) return;
    _foreground = foreground;
    unawaited(sync());
  }

  void dispose() {
    if (!available) return;
    WidgetsBinding.instance.removeObserver(this);
    _watch?.cancel();
    unawaited(_events?.cancel());
    state.dispose();
  }
}
