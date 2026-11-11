import 'dart:async';
import 'dart:io';

import 'package:flutter/widgets.dart';
import 'package:mem_radio/mem_radio.dart';

import 'rust/api.dart';

/// Connects the Bluetooth radio plugin to the node: links and bytes go to Rust,
/// which frames the encounter protocol and answers with commands for the radio.
///
/// The radio runs while Bluetooth is on in MEM and the app is in the foreground, or in the
/// background while "Stay reachable" is on, at low power. Android only for now.
class BleBridge with WidgetsBindingObserver {
  BleBridge(this.node, {required this.reachable});

  final MemNode node;

  /// Whether "Stay reachable in the background" is on.
  final bool Function() reachable;

  /// What the radio last reported, for the Channels screen.
  final radioState = ValueNotifier<RadioState>(RadioState.stopped);

  static bool get available => Platform.isAndroid;

  // A push wake-up starts the engine without a screen: that is not the foreground
  // (Bluetooth in the background only with "Stay reachable").
  bool _foreground =
      WidgetsBinding.instance.lifecycleState == AppLifecycleState.resumed;
  // Radio events reach Rust strictly in order: a link's bytes never overtake its "up".
  Future<void> _chain = Future.value();
  StreamSubscription<RadioEvent>? _radio;
  StreamSubscription<BleCommandItem>? _commands;

  void attach() {
    if (!available) return;
    WidgetsBinding.instance.addObserver(this);
    _radio = MemRadio.events.listen(
      (e) => _chain = _chain.then((_) => _onRadio(e)).catchError((Object _) {}),
    );
    _commands = node.bleCommands().listen(_onCommand);
    unawaited(sync());
  }

  Future<void> _onRadio(RadioEvent e) async {
    switch (e) {
      case LinkUp(:final link):
        await node.bleLinkUp(link: BigInt.from(link));
      case LinkBytes(:final link, :final data):
        await node.bleReceived(link: BigInt.from(link), bytes: data);
      case LinkDown(:final link):
        await node.bleLinkDown(link: BigInt.from(link));
      case Sent(:final op, :final ok):
        await node.bleSent(op: BigInt.from(op), ok: ok);
      case RadioStateChanged(state: final s):
        radioState.value = s;
        if (s != RadioState.running) await node.bleStopped();
      // Wi-Fi Direct events belong to WifiDirectBridge.
      case WifiPeer() || WifiStateChanged() || SoundIn():
        break;
    }
  }

  void _onCommand(BleCommandItem c) {
    switch (c) {
      case BleCommandItem_Send(:final link, :final op, :final bytes):
        unawaited(MemRadio.send(link.toInt(), op.toInt(), bytes));
      case BleCommandItem_Close(:final link):
        unawaited(MemRadio.close(link.toInt()));
    }
  }

  /// Starts, slows or stops the radio to match the setting, the permissions and the app state.
  Future<void> sync() async {
    if (!available) return;
    final on = node.bleEnabled() && (_foreground || reachable());
    if (!on) {
      await MemRadio.stop();
      return;
    }
    final status = await MemRadio.status();
    if (!status.permissions) {
      radioState.value = RadioState.noPermission;
      return;
    }
    if (status.running) {
      await MemRadio.setLowPower(on: !_foreground);
    } else {
      await MemRadio.start(lowPower: !_foreground);
    }
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
    unawaited(_radio?.cancel());
    unawaited(_commands?.cancel());
    radioState.dispose();
  }
}
