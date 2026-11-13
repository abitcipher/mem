import 'dart:io';

import 'package:material_ui/material_ui.dart';
import 'package:video_player/video_player.dart';

import 'media_kind.dart';

/// Texts the player shows, supplied by the app (so this package needs no localizations).
class MemPlayerTexts {
  /// Creates the texts.
  const MemPlayerTexts({
    required this.play,
    required this.pause,
    required this.cannotPlay,
  });

  /// Label of the play button.
  final String play;

  /// Label of the pause button.
  final String pause;

  /// Shown when the engine cannot play the file.
  final String cannotPlay;
}

/// A full-screen player for one local file: video with its picture, audio as a compact panel.
class MemVideoPlayer extends StatefulWidget {
  /// Plays [file] as [kind]; [title] is shown at the top.
  const MemVideoPlayer({
    required this.file,
    required this.kind,
    required this.title,
    required this.texts,
    this.actions = const [],
    super.key,
  });

  /// Whether this platform can play files here at all.
  static bool get isSupported => platformPlaysMedia;

  /// The file to play.
  final File file;

  /// How to show it.
  final MemMediaKind kind;

  /// Title for the app bar.
  final String title;

  /// Texts in the app's language.
  final MemPlayerTexts texts;

  /// Extra app bar actions (for example, open elsewhere).
  final List<Widget> actions;

  @override
  State<MemVideoPlayer> createState() => _MemVideoPlayerState();
}

class _MemVideoPlayerState extends State<MemVideoPlayer> {
  late final VideoPlayerController _controller = VideoPlayerController.file(
    widget.file,
  );
  bool _failed = false;

  @override
  void initState() {
    super.initState();
    _controller.initialize().then(
      (_) {
        if (!mounted) return;
        setState(() {});
        _controller.play();
      },
      onError: (Object _) {
        if (mounted) setState(() => _failed = true);
      },
    );
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final video = widget.kind == MemMediaKind.video;
    return Scaffold(
      backgroundColor: video ? Colors.black : null,
      appBar: AppBar(
        backgroundColor: video ? Colors.black : null,
        foregroundColor: video ? Colors.white : null,
        title: Text(widget.title, maxLines: 1, overflow: TextOverflow.ellipsis),
        actions: widget.actions,
      ),
      body: SafeArea(
        child: _failed
            ? Center(
                child: Padding(
                  padding: const EdgeInsets.all(24),
                  child: Text(
                    widget.texts.cannotPlay,
                    textAlign: TextAlign.center,
                    style: TextStyle(color: video ? Colors.white : null),
                  ),
                ),
              )
            : Column(
                children: [
                  Expanded(
                    child: Center(
                      child: !_controller.value.isInitialized
                          ? const CircularProgressIndicator()
                          : video
                          ? AspectRatio(
                              aspectRatio: _controller.value.aspectRatio,
                              child: VideoPlayer(_controller),
                            )
                          : Icon(
                              Icons.graphic_eq,
                              size: 96,
                              color: Theme.of(context).colorScheme.primary,
                            ),
                    ),
                  ),
                  MemPlayerControls(
                    controller: _controller,
                    texts: widget.texts,
                    light: video,
                  ),
                ],
              ),
      ),
    );
  }
}

/// Play or pause, a seek bar and the times, for any [VideoPlayerController].
class MemPlayerControls extends StatelessWidget {
  /// Controls for [controller]; [light] draws them for a dark background.
  const MemPlayerControls({
    required this.controller,
    required this.texts,
    this.light = false,
    super.key,
  });

  /// The player being controlled.
  final VideoPlayerController controller;

  /// Texts in the app's language.
  final MemPlayerTexts texts;

  /// White controls on a dark background.
  final bool light;

  @override
  Widget build(BuildContext context) {
    final color = light ? Colors.white : null;
    return ValueListenableBuilder<VideoPlayerValue>(
      valueListenable: controller,
      builder: (context, v, _) {
        final total = v.duration.inMilliseconds;
        final at = v.position.inMilliseconds.clamp(0, total > 0 ? total : 0);
        return Padding(
          padding: const EdgeInsets.fromLTRB(8, 0, 16, 8),
          child: Row(
            children: [
              IconButton(
                tooltip: v.isPlaying ? texts.pause : texts.play,
                color: color,
                icon: Icon(v.isPlaying ? Icons.pause : Icons.play_arrow),
                onPressed: v.isInitialized
                    ? () => v.isPlaying ? controller.pause() : controller.play()
                    : null,
              ),
              Expanded(
                child: Slider(
                  value: total > 0 ? at / total : 0,
                  onChanged: total > 0
                      ? (x) => controller.seekTo(
                          Duration(milliseconds: (x * total).round()),
                        )
                      : null,
                ),
              ),
              Text(
                '${formatPosition(v.position)} / ${formatPosition(v.duration)}',
                style: TextStyle(color: color, fontSize: 12),
              ),
            ],
          ),
        );
      },
    );
  }
}

/// "m:ss", or "h:mm:ss" from an hour on.
String formatPosition(Duration d) {
  final s = d.inSeconds % 60;
  final m = d.inMinutes % 60;
  String two(int n) => n.toString().padLeft(2, '0');
  return d.inHours > 0 ? '${d.inHours}:${two(m)}:${two(s)}' : '$m:${two(s)}';
}
