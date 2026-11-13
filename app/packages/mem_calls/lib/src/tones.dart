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
/// Starting takes several steps; [stop] cancels a start still under way, so a tone never
/// begins after it was stopped.
class MemTonePlayer {
  final _player = AudioPlayer()..audioCache = AudioCache(prefix: '');

  /// Counts starts and stops: a start whose number is no longer current gives up.
  int _generation = 0;

  /// Plays [tone] at [volume] (0..1), in a loop or once.
  Future<void> play(
    MemTone tone, {
    required double volume,
    bool loop = true,
  }) async {
    final mine = ++_generation;
    bool stale() => mine != _generation;
    await _player.stop();
    if (volume <= 0 || stale()) return;
    await _player.setAudioContext(
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
    await _player.setReleaseMode(loop ? ReleaseMode.loop : ReleaseMode.stop);
    await _player.setVolume(volume.clamp(0, 1));
    if (stale()) return;
    await _player.play(AssetSource(tone._asset));
    // A stop that came while play() was starting the sound.
    if (stale()) await _player.stop();
  }

  /// Stops whatever plays, and any start still under way.
  Future<void> stop() {
    _generation++;
    return _player.stop();
  }

  /// Releases the player.
  Future<void> dispose() => _player.dispose();
}
