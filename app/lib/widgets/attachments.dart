import 'dart:io';
import 'dart:typed_data';

import 'package:file_selector/file_selector.dart';
import 'package:material_ui/material_ui.dart';
import 'package:mem_video_player/mem_video_player.dart';
import 'package:path_provider/path_provider.dart';
import 'package:share_plus/share_plus.dart';

import '../app_scope.dart';
import '../l10n/gen/app_localizations.dart';
import '../src/rust/api.dart';
import '../theme.dart';

/// Lets the user pick files and sends them to [contactId]. Returns the queued message's id,
/// or `null` if nothing was sent (cancelled or refused, with a note shown).
Future<String?> pickAndSendFiles(
  BuildContext context, {
  required String contactId,
}) async {
  final picked = await openFiles();
  if (picked.isEmpty || !context.mounted) return null;
  final state = AppScope.read(context);
  try {
    final files = <FileInput>[];
    for (final f in picked) {
      final mediaType = f.mimeType ?? mediaTypeFor(f.name);
      // Large files go by path: the node seals them from disk, they never cross in memory.
      final large = await f.length() > maxSmallFile && f.path.isNotEmpty;
      files.add(
        FileInput(
          name: f.name,
          mediaType: mediaType,
          data: large ? Uint8List(0) : await f.readAsBytes(),
          path: large ? f.path : null,
        ),
      );
    }
    final sent = await state.node.sendFiles(
      contactId: contactId,
      text: '',
      files: files,
    );
    state.changed();
    return sent.messageId;
  } on Object catch (e) {
    if (context.mounted) showSnack(context, reasonText(context, e));
    return null;
  }
}

/// Largest file sent with its message in parts; larger ones are pulled directly by the
/// recipient (a file of up to this size fits the 256 KiB sealed bucket).
const maxSmallFile = 256 * 1024 - 1;

/// Largest image decrypted into memory to show in the chat; larger ones open elsewhere.
const maxInlineImage = 16 * 1024 * 1024;

/// A media type from a file name, for platforms whose picker does not say.
String mediaTypeFor(String name) {
  final dot = name.lastIndexOf('.');
  final ext = dot < 0 ? '' : name.substring(dot + 1).toLowerCase();
  return switch (ext) {
    'jpg' || 'jpeg' => 'image/jpeg',
    'png' => 'image/png',
    'gif' => 'image/gif',
    'webp' => 'image/webp',
    'heic' => 'image/heic',
    'pdf' => 'application/pdf',
    'txt' => 'text/plain',
    'mp3' => 'audio/mpeg',
    'ogg' || 'opus' => 'audio/ogg',
    'm4a' => 'audio/mp4',
    'mp4' => 'video/mp4',
    'zip' => 'application/zip',
    _ => 'application/octet-stream',
  };
}

/// Formats a size in bytes for people: "820 B", "14 KB", "1.2 MB".
String formatSize(int bytes) {
  if (bytes < 1024) return '$bytes B';
  if (bytes < 1024 * 1024) return '${(bytes / 1024).round()} KB';
  return '${(bytes / (1024 * 1024)).toStringAsFixed(1)} MB';
}

/// Decrypted files of recent bubbles, so scrolling does not decrypt them again. Bounded: the
/// oldest entries go first.
class _OpenedCache {
  static const _max = 24;
  // Map literals keep insertion order: the first key is the least recently used.
  static final _files = <String, OpenedFile>{};

  static OpenedFile? get(String key) {
    final f = _files.remove(key);
    if (f != null) _files[key] = f;
    return f;
  }

  static void put(String key, OpenedFile file) {
    _files[key] = file;
    while (_files.length > _max) {
      _files.remove(_files.keys.first);
    }
  }
}

/// Opens attachment [index] of [messageId], from the cache when it was opened recently.
Future<OpenedFile?> openAttachment(
  BuildContext context,
  String messageId,
  int index,
) async {
  final key = '$messageId/$index';
  final cached = _OpenedCache.get(key);
  if (cached != null) return cached;
  final opened = await AppScope.read(context).node
      .openAttachment(messageId: messageId, index: index);
  if (opened != null) _OpenedCache.put(key, opened);
  return opened;
}

/// The attached files of one message, inside its bubble.
class AttachmentList extends StatelessWidget {
  const AttachmentList({
    required this.item,
    required this.background,
    super.key,
  });

  final MessageItem item;
  final Color background;

  @override
  Widget build(BuildContext context) {
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        for (var i = 0; i < item.attachments.length; i++)
          Padding(
            padding: const EdgeInsets.only(bottom: 6),
            child: _AttachmentTile(
              messageId: item.id,
              index: i,
              attachment: item.attachments[i],
              background: background,
            ),
          ),
      ],
    );
  }
}

class _AttachmentTile extends StatelessWidget {
  const _AttachmentTile({
    required this.messageId,
    required this.index,
    required this.attachment,
    required this.background,
  });

  final String messageId;
  final int index;
  final AttachmentItem attachment;
  final Color background;

  bool get _image => attachment.mediaType.startsWith('image/');

  bool get _inline => _image && attachment.size.toInt() <= maxInlineImage;

  @override
  Widget build(BuildContext context) {
    final ready = attachment.state is AttachmentStatus_Ready;
    if (ready && _inline) {
      return _ImagePreview(
        messageId: messageId,
        index: index,
        name: attachment.name,
      );
    }
    final l = AppLocalizations.of(context);
    final ink = MemColors.onBubble(background);
    final (String status, double? progress) = switch (attachment.state) {
      AttachmentStatus_Sending(:final sent, :final total) => (
        l.attachmentSending(sent, total),
        total == 0 ? null : sent / total,
      ),
      AttachmentStatus_Receiving(:final have, :final total) => (
        l.attachmentArriving(have, total),
        total == 0 ? null : have / total,
      ),
      AttachmentStatus_Ready() => (formatSize(attachment.size.toInt()), null),
    };
    final arriving = attachment.state is AttachmentStatus_Receiving;
    return InkWell(
      borderRadius: BorderRadius.circular(8),
      onTap: arriving ? null : () => _open(context),
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: 4),
        child: Row(
          children: [
            Icon(switch (MemMediaKind.of(attachment.mediaType)) {
              MemMediaKind.video => Icons.play_circle_outline,
              MemMediaKind.audio => Icons.audiotrack_outlined,
              null when _image => Icons.image_outlined,
              null => Icons.insert_drive_file_outlined,
            }, color: ink),
            const SizedBox(width: 8),
            Flexible(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                mainAxisSize: MainAxisSize.min,
                children: [
                  Text(
                    attachment.name,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(color: ink, fontWeight: FontWeight.w500),
                  ),
                  Text(status, style: TextStyle(color: ink, fontSize: 12)),
                  if (progress != null)
                    Padding(
                      padding: const EdgeInsets.only(top: 4),
                      child: SizedBox(
                        width: 120,
                        child: LinearProgressIndicator(value: progress),
                      ),
                    ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }

  Future<void> _open(BuildContext context) async {
    final l = AppLocalizations.of(context);
    final kind = MemMediaKind.of(attachment.mediaType);
    if (kind != null &&
        MemVideoPlayer.isSupported &&
        AppScope.read(context).settings.builtInPlayer) {
      await playAttachment(context, messageId, index, attachment, kind);
      return;
    }
    if (!_inline) {
      await exportAttachment(context, messageId, index, attachment);
      return;
    }
    final file = await openAttachment(context, messageId, index);
    if (!context.mounted) return;
    if (file == null) {
      showSnack(context, l.attachmentMissing);
      return;
    }
    await shareAttachment(context, file);
  }
}

/// Asks before a file leaves MEM unencrypted for another app.
Future<bool> _confirmOpenElsewhere(BuildContext context) async {
  final l = AppLocalizations.of(context);
  final ok = await showDialog<bool>(
    context: context,
    builder: (context) => AlertDialog(
      title: Text(l.attachmentOpenElsewhere),
      content: Text(l.attachmentOpenElsewhereBody),
      actions: [
        TextButton(
          onPressed: () => Navigator.pop(context, false),
          child: Text(l.cancel),
        ),
        FilledButton(
          onPressed: () => Navigator.pop(context, true),
          child: Text(l.attachmentOpenElsewhereAction),
        ),
      ],
    ),
  );
  return ok == true;
}

/// Hands a decrypted file to another app through the system, after saying what that means.
Future<void> shareAttachment(BuildContext context, OpenedFile file) async {
  if (!await _confirmOpenElsewhere(context)) return;
  await SharePlus.instance.share(
    ShareParams(
      files: [
        XFile.fromData(
          Uint8List.fromList(file.data),
          name: file.name,
          mimeType: file.mediaType,
        ),
      ],
      fileNameOverrides: [file.name],
    ),
  );
}

/// Decrypts a file into a private temporary folder and hands it to another app. Only the
/// latest exported file is kept there.
Future<void> exportAttachment(
  BuildContext context,
  String messageId,
  int index,
  AttachmentItem attachment,
) async {
  final l = AppLocalizations.of(context);
  if (!await _confirmOpenElsewhere(context) || !context.mounted) return;
  final node = AppScope.read(context).node;
  final dir = Directory('${(await getTemporaryDirectory()).path}/opened');
  if (await dir.exists()) await dir.delete(recursive: true);
  await dir.create(recursive: true);
  // Only the last path segment of the sender's name, so it cannot point elsewhere.
  final name = attachment.name.split(RegExp(r'[/\\]')).last;
  final path = '${dir.path}/${name.isEmpty ? 'file' : name}';
  try {
    final done = await node.exportAttachment(
      messageId: messageId,
      index: index,
      path: path,
    );
    if (!context.mounted) return;
    if (!done) {
      showSnack(context, l.attachmentMissing);
      return;
    }
    await SharePlus.instance.share(
      ShareParams(files: [XFile(path, mimeType: attachment.mediaType)]),
    );
  } on Object catch (e) {
    if (context.mounted) showSnack(context, reasonText(context, e));
  }
}

/// Private folder for files being played; cleared after each playback and at start-up, so no
/// decrypted copy outlives its use.
Future<Directory> _playingDir() async =>
    Directory('${(await getTemporaryDirectory()).path}/playing');

/// Removes decrypted copies left by an earlier run (playing, or opened in another app).
Future<void> clearDecryptedCopies() async {
  final tmp = (await getTemporaryDirectory()).path;
  for (final name in ['playing', 'opened']) {
    final dir = Directory('$tmp/$name');
    if (await dir.exists()) await dir.delete(recursive: true);
  }
}

/// Plays a video or audio attachment inside MEM: decrypted into a private temporary file that
/// is deleted when the player closes. Nothing leaves the app.
Future<void> playAttachment(
  BuildContext context,
  String messageId,
  int index,
  AttachmentItem attachment,
  MemMediaKind kind,
) async {
  final l = AppLocalizations.of(context);
  final node = AppScope.read(context).node;
  final dir = await _playingDir();
  await dir.create(recursive: true);
  final file = File('${dir.path}/$messageId-$index');
  try {
    final done = await node.exportAttachment(
      messageId: messageId,
      index: index,
      path: file.path,
    );
    if (!context.mounted) return;
    if (!done) {
      showSnack(context, l.attachmentMissing);
      return;
    }
    await Navigator.of(context).push(
      MaterialPageRoute<void>(
        builder: (context) => MemVideoPlayer(
          file: file,
          kind: kind,
          title: attachment.name,
          texts: MemPlayerTexts(
            play: l.playerPlay,
            pause: l.playerPause,
            cannotPlay: l.playerCannotPlay,
          ),
          actions: [
            IconButton(
              tooltip: l.attachmentOpenElsewhere,
              icon: const Icon(Icons.share),
              onPressed: () =>
                  exportAttachment(context, messageId, index, attachment),
            ),
          ],
        ),
      ),
    );
  } on Object catch (e) {
    if (context.mounted) showSnack(context, reasonText(context, e));
  } finally {
    if (await file.exists()) await file.delete();
  }
}

/// A received or sent image, shown inside the bubble; tap for full screen.
class _ImagePreview extends StatelessWidget {
  const _ImagePreview({
    required this.messageId,
    required this.index,
    required this.name,
  });

  final String messageId;
  final int index;
  final String name;

  @override
  Widget build(BuildContext context) {
    return FutureBuilder<OpenedFile?>(
      future: openAttachment(context, messageId, index),
      builder: (context, snap) {
        final file = snap.data;
        if (file == null) {
          return const SizedBox(
            height: 120,
            child: Center(child: CircularProgressIndicator()),
          );
        }
        final bytes = Uint8List.fromList(file.data);
        return GestureDetector(
          onTap: () => Navigator.of(context).push(
            MaterialPageRoute<void>(
              builder: (_) => ImageViewer(file: file, bytes: bytes),
            ),
          ),
          child: ClipRRect(
            borderRadius: BorderRadius.circular(8),
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxHeight: 240, maxWidth: 260),
              child: Image.memory(
                bytes,
                fit: BoxFit.cover,
                semanticLabel: name,
                errorBuilder: (_, _, _) => const Icon(Icons.broken_image),
              ),
            ),
          ),
        );
      },
    );
  }
}

/// One image, full screen, with zoom; the share button hands it to another app.
class ImageViewer extends StatelessWidget {
  const ImageViewer({required this.file, required this.bytes, super.key});

  final OpenedFile file;
  final Uint8List bytes;

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return Scaffold(
      backgroundColor: Colors.black,
      appBar: AppBar(
        backgroundColor: Colors.black,
        foregroundColor: Colors.white,
        title: Text(file.name, maxLines: 1, overflow: TextOverflow.ellipsis),
        actions: [
          IconButton(
            tooltip: l.attachmentOpenElsewhere,
            icon: const Icon(Icons.share),
            onPressed: () => shareAttachment(context, file),
          ),
        ],
      ),
      body: InteractiveViewer(
        maxScale: 6,
        child: Center(child: Image.memory(bytes, semanticLabel: file.name)),
      ),
    );
  }
}
