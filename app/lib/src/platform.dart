import 'dart:io' as io;

import 'package:file_selector/file_selector.dart' as fs;
import 'package:flutter/services.dart';

/// Platform services for backups: capture protection for the recovery phrase, and
/// files through the system pickers. Android uses the Storage Access Framework (no storage
/// permission); Linux uses the desktop file dialogs and plain paths.
abstract final class PlatformServices {
  static const _channel = MethodChannel('mem/platform');
  static const _maxFile = 64 * 1024 * 1024 + 128;
  static bool get _android => io.Platform.isAndroid;

  static var _secureScreens = 0;
  static var _secureApp = false;

  /// Blocks screenshots and screen recording while `on` (Android `FLAG_SECURE`), for screens that
  /// show secrets (the recovery phrase). Calls nest, and the app-wide setting stays in force.
  /// No-op elsewhere.
  static Future<void> setSecure({required bool on}) async {
    final before = _secure;
    _secureScreens = (on ? _secureScreens + 1 : _secureScreens - 1).clamp(
      0,
      1 << 20,
    );
    await _applySecure(before);
  }

  /// The user setting that blocks screenshots on every screen.
  static Future<void> setSecureApp({required bool on}) async {
    final before = _secure;
    _secureApp = on;
    await _applySecure(before);
  }

  static bool get _secure => _secureApp || _secureScreens > 0;

  static Future<void> _applySecure(bool before) async {
    if (!_android || before == _secure) return;
    await _channel.invokeMethod<void>('setSecure', _secure);
  }

  static var _codes = 0;
  static var _codeBrightness = 1.0;

  /// Screen brightness and no screen timeout while a QR code is shown (Android): the user's
  /// [setCodeBrightness], full by default. Calls nest: the screen returns to normal when the last
  /// code is gone. No-op elsewhere.
  static Future<void> showingCode({required bool on}) async {
    final before = _codes;
    _codes = (on ? _codes + 1 : _codes - 1).clamp(0, 1 << 20);
    if (!_android || (before > 0) == (_codes > 0)) return;
    await _channel.invokeMethod<void>(
      'setShowingCode',
      _codes > 0 ? _codeBrightness : null,
    );
  }

  /// The brightness for shown codes, from 0.05 to 1; applied at once while one is shown.
  static Future<void> setCodeBrightness(double value) async {
    _codeBrightness = value.clamp(0.05, 1.0);
    if (!_android || _codes == 0) return;
    await _channel.invokeMethod<void>('setShowingCode', _codeBrightness);
  }

  /// Starts or stops the background foreground service (Android), with localized
  /// notification text. No-op elsewhere.
  static Future<void> setReachable({
    required bool on,
    required String title,
    required String text,
    bool ble = false,
  }) async {
    if (!_android) return;
    await _channel.invokeMethod<void>('setReachable', {
      'on': on,
      'title': title,
      'text': text,
      'ble': ble,
    });
  }

  /// Asks for permission to post notifications (Android 13+). No-op elsewhere.
  static Future<void> askNotifications() async {
    if (!_android) return;
    await _channel.invokeMethod<void>('askNotifications');
  }

  /// Whether Android exempts MEM from battery optimization. True elsewhere.
  static Future<bool> batteryUnrestricted() async {
    if (!_android) return true;
    return await _channel.invokeMethod<bool>('batteryUnrestricted') ?? false;
  }

  /// Opens the system dialog that exempts MEM from battery optimization.
  static Future<void> requestBatteryUnrestricted() async {
    if (!_android) return;
    await _channel.invokeMethod<void>('requestBatteryUnrestricted');
  }

  /// Lets the user save `bytes` under a suggested `name`. Returns false if cancelled.
  static Future<bool> saveFile(String name, Uint8List bytes) async {
    if (_android) {
      return await _channel.invokeMethod<bool>('saveDocument', {
            'name': name,
            'bytes': bytes,
          }) ??
          false;
    }
    final location = await fs.getSaveLocation(suggestedName: name);
    if (location == null) return false;
    await io.File(location.path).writeAsBytes(bytes, flush: true);
    return true;
  }

  /// Lets the user pick a file and returns its bytes (at most 64 MiB), or null.
  static Future<Uint8List?> openFile() async {
    if (_android) return _channel.invokeMethod<Uint8List>('openDocument');
    final file = await fs.openFile();
    if (file == null || await file.length() > _maxFile) return null;
    return file.readAsBytes();
  }

  /// Lets the user pick a folder for automatic backups. Returns an opaque location (an Android
  /// tree URI or a desktop path), or null.
  static Future<String?> pickFolder() async {
    if (_android) return _channel.invokeMethod<String>('pickFolder');
    return fs.getDirectoryPath();
  }

  /// Writes `bytes` as `name` into a folder from [pickFolder]. Returns false on failure.
  static Future<bool> writeToFolder(
    String folder,
    String name,
    Uint8List bytes,
  ) async {
    if (_android) {
      return await _channel.invokeMethod<bool>('writeToFolder', {
            'folder': folder,
            'name': name,
            'bytes': bytes,
          }) ??
          false;
    }
    try {
      await io.File('$folder${io.Platform.pathSeparator}$name')
          .writeAsBytes(bytes, flush: true);
      return true;
    } on io.FileSystemException {
      return false;
    }
  }

  /// Text another app handed to MEM ("Share → MEM", or MEM in the selected-text menu),
  /// once; null if none. Android only.
  static Future<String?> takeInboundText() async {
    if (!_android) return null;
    return _channel.invokeMethod<String>('takeInboundText');
  }

  /// Called when text arrives while MEM is open (Android).
  static void onInboundText(void Function() handler) {
    if (!_android) return;
    const MethodChannel('mem/inbound').setMethodCallHandler((call) async {
      if (call.method == 'inboundText') handler();
    });
  }

  /// The installed Telegram app's package for "Send via Telegram", or null.
  static Future<String?> telegramApp() async {
    if (!_android) return null;
    return _channel.invokeMethod<String>('telegramApp');
  }

  /// Hands `text` straight to the Telegram app `package`; true if it opened.
  static Future<bool> shareTo(String package, String text) async {
    if (!_android) return false;
    return await _channel.invokeMethod<bool>('shareTo', {
          'package': package,
          'text': text,
        }) ??
        false;
  }
}
