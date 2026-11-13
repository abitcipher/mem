/// Plays local video and audio files: a full-screen video player and a compact audio player,
/// on the platform's own media engine (ExoPlayer on Android, AVPlayer on iOS and macOS).
///
/// The package knows nothing about where files come from. Callers check [MemMediaKind.of] and
/// [MemVideoPlayer.isSupported] first, and otherwise open files another way.
library;

export 'src/media_kind.dart';
export 'src/player.dart';
