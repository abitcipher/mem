import 'package:flutter/foundation.dart';

/// What a file plays as.
enum MemMediaKind {
  /// A moving picture, usually with sound.
  video,

  /// Sound only.
  audio;

  /// The kind for [mediaType] when the platform engine is expected to play it, else `null`.
  ///
  /// Only common phone formats are claimed; anything else is better opened in another app
  /// than failing here.
  static MemMediaKind? of(String mediaType) {
    final type = mediaType.toLowerCase().split(';').first.trim();
    if (_video.contains(type)) return MemMediaKind.video;
    if (_audio.contains(type)) return MemMediaKind.audio;
    return null;
  }

  static const _video = {
    'video/mp4',
    'video/3gpp',
    'video/quicktime',
    'video/webm',
  };
  static const _audio = {
    'audio/mpeg',
    'audio/mp4',
    'audio/aac',
    'audio/ogg',
    'audio/opus',
    'audio/wav',
    'audio/x-wav',
    'audio/flac',
  };
}

/// Whether this platform has an engine for local files (`video_player` supports Android, iOS
/// and macOS; on Linux and Windows files open in another app).
bool get platformPlaysMedia =>
    !kIsWeb &&
    (defaultTargetPlatform == TargetPlatform.android ||
        defaultTargetPlatform == TargetPlatform.iOS ||
        defaultTargetPlatform == TargetPlatform.macOS);
