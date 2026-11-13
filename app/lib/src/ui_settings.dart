import 'dart:io';

import '../widgets/grade_symbol.dart';
import 'rust/api.dart';

/// Interface language: the phone's, or a fixed one.
enum AppLanguage { system, en, uk }

/// Which sound pairing by sound uses.
enum SoundTone { audible, inaudible }

/// How much a message's full channel badges say.
enum BadgeDetail { compact, detailed }

/// App settings that live only in the UI: stored as `key=value` lines in a small file next to
/// the profile. No secret material. Each value has a default and is bounded on read, so a damaged
/// file only resets settings. Security-relevant limits stay in the core.
class UiSettings {
  UiSettings._(this._file);

  final File? _file;

  /// Bounds for the text size, in percent of the normal size.
  static const minTextPercent = 85;
  static const maxTextPercent = 150;

  /// How densely and quickly animated QR codes are shown.
  QrPace qrPace = QrPace.normal;

  /// Text size in percent; `null` follows the system setting.
  int? textPercent;

  AppLanguage language = AppLanguage.system;

  /// Blocks screenshots and screen recording on every screen (Android).
  bool blockScreenshots = false;

  BadgeDetail badgeDetail = BadgeDetail.compact;

  /// Positions of the message badges slider, left to right.
  static const minBadgeMode = 1;
  static const maxBadgeMode = 14;

  /// Message badges: 1 shows only the symbols of P4, P5 and grades below the contact's floor;
  /// 2 shows symbols only; 3 to 13 show the full badges, then collapse them to symbols after
  /// [badgeDelay]; 14 always shows the full badges.
  int badgeMode = 5;

  /// Collapse delays for slider positions 3 to 13.
  static const _badgeDelays = [
    3,
    7,
    15,
    37,
    60,
    180,
    300,
    420,
    1020,
    1800,
    3600,
  ];

  /// How long full badges stay before collapsing, for [mode]; `null` when they never show full by
  /// themselves (1, 2) or never collapse (14).
  static Duration? badgeDelay(int mode) => mode >= 3 && mode <= 13
      ? Duration(seconds: _badgeDelays[mode - 3])
      : null;

  /// How grades map to symbol shapes.
  SymbolStyle symbolStyle = SymbolStyle.byGrade;

  /// The shape for P0 to P3 when [symbolStyle] is [SymbolStyle.oneShape].
  SymbolShape symbolShape = SymbolShape.fisheye;

  /// Time under each message bubble.
  bool showMessageTimes = true;

  /// Video and audio attachments play inside MEM (where the platform can); off, they open in
  /// another app.
  bool builtInPlayer = true;

  /// Voice calls with contacts (being tested; off by default).
  bool voiceCalls = false;

  /// Calls are kept as lines in the chat.
  bool callHistory = true;

  /// Tone for incoming calls (a `MemTone` id, or `off`) and its volume (0..1).
  String incomingTone = 'chime';
  double incomingVolume = 0.8;

  /// Ringback while our call rings (a `MemTone` id, or `off`) and its volume (0..1).
  String outgoingTone = 'ringback';
  double outgoingVolume = 0.6;

  /// The local network status button in the Home toolbar.
  bool showNetworkStatus = true;

  /// Keep MEM running with the screen off (Android foreground service).
  bool stayReachable = false;

  /// Wake-ups through a UnifiedPush distributor. Off until the user turns it on.
  bool push = false;

  /// Pairing by sound: audible tones, or near-ultrasound.
  SoundTone soundTone = SoundTone.audible;

  /// Bound for [scanExposure], in exposure steps (EV) either way.
  static const maxScanExposure = 4.0;

  /// Brightness correction for the QR scanner, in EV: negative darkens a bright phone screen
  /// that the camera overexposes. Clamped to what each camera supports.
  double scanExposure = 0;

  /// Lowest screen brightness for a shown code, as a fraction of full.
  static const minCodeBrightness = 0.1;

  /// Screen brightness while a QR code is shown: full by default; lower when the other camera
  /// washes the code out.
  double codeBrightness = 1;

  /// Reads the settings from `file`; `null` keeps them in memory only (tests).
  static UiSettings read(File? file) {
    final s = UiSettings._(file);
    T pick<T extends Enum>(List<T> values, String v, T fallback) =>
        values.firstWhere((e) => e.name == v, orElse: () => fallback);
    try {
      for (final line in file?.readAsLinesSync() ?? const <String>[]) {
        final i = line.indexOf('=');
        if (i <= 0) continue;
        final (k, v) = (line.substring(0, i), line.substring(i + 1));
        switch (k) {
          case 'qr_pace':
            s.qrPace = pick(QrPace.values, v, QrPace.normal);
          case 'text_percent':
            final p = int.tryParse(v);
            s.textPercent = p?.clamp(minTextPercent, maxTextPercent);
          case 'language':
            s.language = pick(AppLanguage.values, v, AppLanguage.system);
          case 'block_screenshots':
            s.blockScreenshots = v == 'true';
          case 'badge_detail':
            s.badgeDetail = pick(BadgeDetail.values, v, BadgeDetail.compact);
          case 'badge_mode':
            final m = int.tryParse(v);
            if (m != null) s.badgeMode = m.clamp(minBadgeMode, maxBadgeMode);
          case 'symbol_style':
            s.symbolStyle = pick(SymbolStyle.values, v, SymbolStyle.byGrade);
          case 'symbol_shape':
            final shape = pick(SymbolShape.values, v, SymbolShape.fisheye);
            s.symbolShape = oneShapeChoices.contains(shape)
                ? shape
                : SymbolShape.fisheye;
          case 'message_times':
            s.showMessageTimes = v != 'false';
          case 'media_player':
            s.builtInPlayer = v != 'false';
          case 'voice_calls':
            s.voiceCalls = v == 'true';
          case 'call_history':
            s.callHistory = v != 'false';
          case 'incoming_tone':
            s.incomingTone = v;
          case 'incoming_volume':
            s.incomingVolume = (double.tryParse(v) ?? 0.8).clamp(0, 1);
          case 'outgoing_tone':
            s.outgoingTone = v;
          case 'outgoing_volume':
            s.outgoingVolume = (double.tryParse(v) ?? 0.6).clamp(0, 1);
          case 'network_status':
            s.showNetworkStatus = v != 'false';
          case 'stay_reachable':
            s.stayReachable = v == 'true';
          case 'push':
            s.push = v == 'true';
          case 'sound_tone':
            s.soundTone = pick(SoundTone.values, v, SoundTone.audible);
          case 'code_brightness':
            final b = double.tryParse(v);
            if (b != null && b.isFinite) {
              s.codeBrightness = b.clamp(minCodeBrightness, 1.0);
            }
          case 'scan_exposure':
            final e = double.tryParse(v);
            if (e != null && e.isFinite) {
              s.scanExposure = e.clamp(-maxScanExposure, maxScanExposure);
            }
        }
      }
    } on FileSystemException {
      // Defaults.
    }
    return s;
  }

  void write() {
    try {
      _file?.writeAsStringSync(
        [
          'qr_pace=${qrPace.name}',
          if (textPercent != null) 'text_percent=$textPercent',
          'language=${language.name}',
          'block_screenshots=$blockScreenshots',
          'badge_detail=${badgeDetail.name}',
          'badge_mode=$badgeMode',
          'symbol_style=${symbolStyle.name}',
          'symbol_shape=${symbolShape.name}',
          'message_times=$showMessageTimes',
          'media_player=$builtInPlayer',
          'voice_calls=$voiceCalls',
          'call_history=$callHistory',
          'incoming_tone=$incomingTone',
          'incoming_volume=$incomingVolume',
          'outgoing_tone=$outgoingTone',
          'outgoing_volume=$outgoingVolume',
          'network_status=$showNetworkStatus',
          'stay_reachable=$stayReachable',
          'push=$push',
          'sound_tone=${soundTone.name}',
          'scan_exposure=$scanExposure',
          'code_brightness=$codeBrightness',
        ].join('\n'),
      );
    } on FileSystemException {
      // Keeps working in memory.
    }
  }
}
