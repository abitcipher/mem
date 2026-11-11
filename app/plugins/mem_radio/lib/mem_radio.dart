/// MEM's radios: Bluetooth LE advertising, scanning and links in both roles.
///
/// The plugin moves bytes only. Framing, the encounter protocol and every decision about what to
/// send live in the Rust core; the app wires [MemRadio.events] to it.
library;

import 'dart:async';

import 'package:flutter/services.dart';

/// Something the radio reports.
sealed class RadioEvent {
  const RadioEvent();
}

/// A link to a nearby MEM device is ready (either role).
class LinkUp extends RadioEvent {
  const LinkUp(this.link);
  final int link;
}

/// Stream bytes arrived on a link, in order.
class LinkBytes extends RadioEvent {
  const LinkBytes(this.link, this.data);
  final int link;
  final Uint8List data;
}

/// A link is gone.
class LinkDown extends RadioEvent {
  const LinkDown(this.link);
  final int link;
}

/// A [MemRadio.send] finished.
class Sent extends RadioEvent {
  const Sent(this.op, {required this.ok});
  final int op;
  final bool ok;
}

/// The radio's state changed.
class RadioStateChanged extends RadioEvent {
  const RadioStateChanged(this.state);
  final RadioState state;
}

enum RadioState { running, stopped, bluetoothOff, unsupported, noPermission }

/// Wi-Fi Direct: joined a group; dial the owner's endpoint `id` at `ip:port`.
class WifiPeer extends RadioEvent {
  const WifiPeer(this.id, this.ip, this.port);
  final String id;
  final String ip;
  final int port;
}

/// Wi-Fi Direct's state changed: "running", "stopped", "unsupported" or "error".
class WifiStateChanged extends RadioEvent {
  const WifiStateChanged(this.state);
  final String state;
}

/// What stands between the user and running Wi-Fi Direct.
/// Microphone samples while pairing by sound: 16-bit mono PCM, little-endian.
class SoundIn extends RadioEvent {
  SoundIn(this.data);
  final Uint8List data;
}

class WifiStatus {
  const WifiStatus({
    required this.supported,
    required this.permissions,
    required this.locationNeeded,
    required this.locationOn,
    required this.running,
  });

  final bool supported;

  /// Android 13+: nearby Wi-Fi devices; Android 6–12: location.
  final bool permissions;

  /// Android 6–12: finding peers needs Location switched on, though MEM never reads one.
  final bool locationNeeded;
  final bool locationOn;
  final bool running;

  bool get ready => supported && permissions && (!locationNeeded || locationOn);
}

/// What stands between the user and a running radio.
class RadioStatus {
  const RadioStatus({
    required this.supported,
    required this.bluetoothOn,
    required this.permissions,
    required this.locationNeeded,
    required this.locationOn,
    required this.running,
  });

  /// The phone has Bluetooth LE.
  final bool supported;

  /// Bluetooth is switched on.
  final bool bluetoothOn;

  /// The Bluetooth permissions (or, on Android 6–11, the location permission) are granted.
  final bool permissions;

  /// Android 6–11: scanning needs Location switched on, though MEM never reads a location.
  final bool locationNeeded;
  final bool locationOn;

  final bool running;

  /// Everything is in place to start.
  bool get ready =>
      supported &&
      bluetoothOn &&
      permissions &&
      (!locationNeeded || locationOn);
}

class MemRadio {
  MemRadio._();

  static const _methods = MethodChannel('mem_radio');
  static const _events = EventChannel('mem_radio/events');

  static Stream<RadioEvent>? _stream;

  /// Radio events. Listen before [start].
  static Stream<RadioEvent> get events => _stream ??= _events
      .receiveBroadcastStream()
      .map(_event)
      .where((e) => e != null)
      .cast();

  static RadioEvent? _event(dynamic raw) {
    if (raw is! Map) return null;
    final link = (raw['link'] as num?)?.toInt();
    return switch (raw['type']) {
      'up' when link != null => LinkUp(link),
      'bytes' when link != null => LinkBytes(link, raw['data'] as Uint8List),
      'down' when link != null => LinkDown(link),
      'sent' => Sent((raw['op'] as num).toInt(), ok: raw['ok'] == true),
      'wifiPeer' => WifiPeer(
        raw['id'] as String,
        raw['ip'] as String,
        (raw['port'] as num).toInt(),
      ),
      'wifiState' => WifiStateChanged(raw['state'] as String),
      'soundIn' => SoundIn(raw['data'] as Uint8List),
      'state' => RadioStateChanged(
        RadioState.values.firstWhere(
          (s) => s.name == raw['state'],
          orElse: () => RadioState.stopped,
        ),
      ),
      _ => null,
    };
  }

  static Future<RadioStatus> status() async {
    final m = await _methods.invokeMapMethod<String, Object?>('status') ?? {};
    return RadioStatus(
      supported: m['supported'] == true,
      bluetoothOn: m['bluetoothOn'] == true,
      permissions: m['permissions'] == true,
      locationNeeded: m['locationNeeded'] == true,
      locationOn: m['locationOn'] == true,
      running: m['running'] == true,
    );
  }

  /// Asks for the missing permissions; `true` if all are granted afterwards.
  static Future<bool> requestPermissions() async =>
      await _methods.invokeMethod<bool>('requestPermissions') ?? false;

  /// Shows the system dialog to switch Bluetooth on.
  static Future<bool> enableBluetooth() async =>
      await _methods.invokeMethod<bool>('enableBluetooth') ?? false;

  static Future<void> openLocationSettings() =>
      _methods.invokeMethod<void>('openLocationSettings');

  /// Starts advertising and scanning; `false` without permissions.
  static Future<bool> start({bool lowPower = false}) async =>
      await _methods.invokeMethod<bool>('start', {'lowPower': lowPower}) ??
      false;

  static Future<void> stop() => _methods.invokeMethod<void>('stop');

  /// Low power while the app is in the background with "Stay reachable" on.
  static Future<void> setLowPower({required bool on}) =>
      _methods.invokeMethod<void>('setLowPower', on);

  /// Writes stream bytes to a link; a [Sent] event with [op] follows.
  static Future<void> send(int link, int op, Uint8List bytes) => _methods
      .invokeMethod<void>('send', {'link': link, 'op': op, 'bytes': bytes});

  static Future<void> close(int link) =>
      _methods.invokeMethod<void>('close', {'link': link});

  // ---- Wi-Fi Direct ----

  static Future<WifiStatus> wifiStatus() async {
    final m =
        await _methods.invokeMapMethod<String, Object?>('wifiStatus') ?? {};
    return WifiStatus(
      supported: m['supported'] == true,
      permissions: m['permissions'] == true,
      locationNeeded: m['locationNeeded'] == true,
      locationOn: m['locationOn'] == true,
      running: m['running'] == true,
    );
  }

  static Future<bool> wifiRequestPermissions() async =>
      await _methods.invokeMethod<bool>('wifiRequestPermissions') ?? false;

  /// Advertises this device's Iroh endpoint and looks for other MEM phones.
  static Future<bool> wifiStart(String id, int port) async =>
      await _methods.invokeMethod<bool>('wifiStart', {
        'id': id,
        'port': port,
      }) ??
      false;

  static Future<void> wifiStop() => _methods.invokeMethod<void>('wifiStop');

  // ---- sound: pairing cards only ----

  /// Asks for the microphone; true when granted.
  static Future<bool> soundRequestPermission() async =>
      await _methods.invokeMethod<bool>('soundRequestPermission') ?? false;

  /// Plays 16-bit mono PCM at 48 kHz once; completes when it has finished.
  static Future<bool> soundPlay(Uint8List pcm) async =>
      await _methods.invokeMethod<bool>('soundPlay', {'pcm': pcm}) ?? false;

  /// Starts sending microphone samples as [SoundIn] events.
  static Future<bool> soundStart() async =>
      await _methods.invokeMethod<bool>('soundStart') ?? false;

  /// Stops the microphone.
  static Future<void> soundStop() => _methods.invokeMethod<void>('soundStop');

  /// Ends the current group once its encounter is over.
  static Future<void> wifiLeaveGroup() =>
      _methods.invokeMethod<void>('wifiLeaveGroup');
}
