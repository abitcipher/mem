import 'package:audioplayers/audioplayers.dart';

/// Built-in call tones: short sounds bundled with the package (nothing is fetched).
enum MemTone {
  /// Three rising bell notes (incoming).
  chime,

  /// A double trill (incoming).
  classic,

  /// Three short beeps (incoming).
  pulse,

  /// A soft swell (incoming).
  soft,

  /// The European ringback: 425 Hz, one second on, four off (outgoing).
  ringback,

  /// A short beep every three seconds (outgoing).
  beep;

  /// Stable id for settings.
  String get id => name;

  /// Whether this is a tone for incoming calls (else a ringback for outgoing ones).
  bool get incoming => index <= MemTone.soft.index;

  String get _asset => 'packages/mem_calls/assets/tones/$name.wav';

  /// The tone with [id], or `null` (for example "off").
  static MemTone? byId(String? id) {
    for (final t in MemTone.values) {
      if (t.id == id) return t;
    }
    return null;
  }

  /// Tones for incoming calls, in the order offered.
  static List<MemTone> get forIncoming =>
      MemTone.values.where((t) => t.incoming).toList();

  /// Tones for outgoing calls, in the order offered.
  static List<MemTone> get forOutgoing =>
      MemTone.values.where((t) => !t.incoming).toList();
}

/// Plays one call tone at a time. Incoming tones go to the phone's ringtone stream, so they
/// follow its ring volume and silent mode; ringbacks go to the call stream.
///
/// Every tone gets its own player, released on [stop]: a released player cannot start, even
/// if the system was still preparing it (on older Android a stopped player could start once
/// its preparation finished, and then ring on).
class MemTonePlayer {
  AudioPlayer? _player;

  /// Counts starts and stops: a start whose number is no longer current gives up.
  int _generation = 0;

  /// Plays [tone] at [volume] (0..1), in a loop or once.
  Future<void> play(
    MemTone tone, {
    required double volume,
    bool loop = true,
  }) async {
    await stop();
    final mine = _generation;
    if (volume <= 0) return;
    final player = AudioPlayer()..audioCache = AudioCache(prefix: '');
    _player = player;
    bool stale() => mine != _generation;
    try {
      await player.setAudioContext(
        AudioContext(
          android: tone.incoming
              ? const AudioContextAndroid(
                  usageType: AndroidUsageType.notificationRingtone,
                  contentType: AndroidContentType.sonification,
                  audioFocus: AndroidAudioFocus.gainTransient,
                )
              : const AudioContextAndroid(
                  usageType: AndroidUsageType.voiceCommunicationSignalling,
                  contentType: AndroidContentType.sonification,
                  audioFocus: AndroidAudioFocus.gainTransient,
                ),
        ),
      );
      if (stale()) return;
      await player.setReleaseMode(
        loop ? ReleaseMode.loop : ReleaseMode.release,
      );
      await player.setVolume(volume.clamp(0, 1));
      if (stale()) return;
      await player.play(AssetSource(tone._asset));
    } finally {
      // A stop that came while this one was starting: make sure it is gone.
      if (stale()) await player.dispose();
    }
  }

  /// Stops and releases whatever plays, and any start still under way.
  Future<void> stop() async {
    _generation++;
    final player = _player;
    _player = null;
    if (player == null) return;
    try {
      await player.stop();
    } finally {
      await player.dispose();
    }
  }

  /// Releases the player.
  Future<void> dispose() => stop();
}
