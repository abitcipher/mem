/// Push wake-ups through UnifiedPush. The plugin moves no message data: a wake-up
/// only means "fetch from your hub now".
library;

import 'package:flutter/services.dart';

class MemPush {
  MemPush._();

  static const _channel = MethodChannel('mem_push');

  /// Sets what to do on events from the distributor: a new endpoint (or `null`), a wake-up.
  static void listen({
    required Future<void> Function(String? endpoint) onEndpoint,
    required Future<void> Function() onWake,
  }) {
    _channel.setMethodCallHandler((call) async {
      switch (call.method) {
        case 'endpoint':
          await onEndpoint(call.arguments as String?);
        case 'wake':
          await onWake();
      }
    });
  }

  /// Installed distributor apps (package names), e.g. ntfy.
  static Future<List<String>> distributors() async =>
      (await _channel.invokeListMethod<String>('distributors')) ?? const [];

  /// Registers with [distributor] (or the saved/default one); the endpoint arrives later.
  static Future<void> register({String? distributor}) =>
      _channel.invokeMethod<void>('register', {'distributor': distributor});

  static Future<void> unregister() => _channel.invokeMethod<void>('unregister');

  /// The last endpoint the distributor gave.
  static Future<String?> endpoint() => _channel.invokeMethod<String>('endpoint');

  /// Whether a wake-up started this engine and is waiting for us.
  static Future<bool> takePendingWake() async =>
      await _channel.invokeMethod<bool>('takePendingWake') ?? false;

  /// The wake-up's fetch is finished.
  static Future<void> wakeDone() => _channel.invokeMethod<void>('wakeDone');

  /// Shows a notification without names or message text.
  static Future<void> notify({required String title, required String text}) =>
      _channel.invokeMethod<void>('notify', {'title': title, 'text': text});
}
