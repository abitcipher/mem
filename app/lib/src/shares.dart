import 'dart:async';
import 'dart:io' as io;

import 'package:file_selector/file_selector.dart' as fs;
import 'package:flutter/services.dart';
import 'package:path_provider/path_provider.dart';

import '../widgets/attachments.dart' show mediaTypeFor;
import 'rust/api.dart';

/// Largest shared file (the attachment limit).
const maxSharedFile = 100 * 1024 * 1024;

/// Most files in one share.
const maxShareFiles = 512;

/// Picking files to share, and reading them when a contact asks. Android keeps a persisted
/// read permission per picked document (or the picked folder) and copies a file into MEM's
/// cache only while it is being sent; desktops use plain paths.
abstract final class SharePicker {
  static const _channel = MethodChannel('mem/platform');
  static bool get _android => io.Platform.isAndroid;

  /// Lets the user pick files; null if cancelled.
  static Future<List<ShareFileInput>?> pickFiles() async {
    if (_android) {
      final list = await _channel.invokeListMethod<Map<Object?, Object?>>(
        'pickShareFiles',
      );
      return list?.map(_fromMap).toList();
    }
    final picked = await fs.openFiles();
    if (picked.isEmpty) return null;
    return [for (final f in picked) await _fromPath(f.path, f.name)];
  }

  /// Lets the user pick a folder; its name and files (subfolders included, up to
  /// [maxShareFiles]), or null if cancelled.
  static Future<({String name, List<ShareFileInput> files})?>
  pickFolder() async {
    if (_android) {
      final m = await _channel.invokeMapMethod<String, Object?>(
        'pickShareFolder',
      );
      if (m == null) return null;
      final files = (m['files'] as List<Object?>? ?? const [])
          .cast<Map<Object?, Object?>>()
          .map(_fromMap)
          .toList();
      return (name: (m['name'] as String?) ?? 'Folder', files: files);
    }
    final dir = await fs.getDirectoryPath();
    if (dir == null) return null;
    final files = <ShareFileInput>[];
    await for (final e in io.Directory(dir).list(recursive: true)) {
      if (files.length >= maxShareFiles) break;
      if (e is io.File) {
        files.add(await _fromPath(e.path, e.uri.pathSegments.last));
      }
    }
    return (
      name: io.Directory(dir).uri.pathSegments.where((s) => s.isNotEmpty).last,
      files: files,
    );
  }

  static ShareFileInput _fromMap(Map<Object?, Object?> m) {
    final name = (m['name'] as String?) ?? 'file';
    final mime = m['mime'] as String?;
    return ShareFileInput(
      name: name,
      size: BigInt.from((m['size'] as num?)?.toInt() ?? 0),
      mediaType: mime == null || mime == 'application/octet-stream'
          ? mediaTypeFor(name)
          : mime,
      modifiedS: switch (m['modified']) {
        final num n => BigInt.from(n.toInt()),
        _ => null,
      },
      handle: m['uri']! as String,
    );
  }

  static Future<ShareFileInput> _fromPath(String path, String name) async {
    final stat = await io.File(path).stat();
    return ShareFileInput(
      name: name,
      size: BigInt.from(stat.size),
      mediaType: mediaTypeFor(name),
      modifiedS: BigInt.from(stat.modified.millisecondsSinceEpoch ~/ 1000),
      handle: path,
    );
  }

  /// A readable path for the file at `handle`, and whether it is a temporary copy to delete
  /// after sending; null when the file went away or lost its permission.
  static Future<({String path, bool temporary})?> readable(
    String handle,
  ) async {
    if (!_android) {
      final f = io.File(handle);
      return await f.exists() ? (path: handle, temporary: false) : null;
    }
    final dir = await getTemporaryDirectory();
    final path =
        '${dir.path}/share-${DateTime.now().microsecondsSinceEpoch}.part';
    final size = await _channel.invokeMethod<int>('copyShared', {
      'uri': handle,
      'path': path,
      'limit': maxSharedFile,
    });
    return size == null ? null : (path: path, temporary: true);
  }
}

/// Answers contacts' requests for our shared files, one at a time, without asking: the
/// share itself was the user's consent. The core checks every request again.
class ShareServer {
  ShareServer(this.node);

  final MemNode node;
  var _busy = false;
  final _queue = <ShareRequestItem>[];

  /// Called when the node says requests are waiting.
  Future<void> onRequests() async {
    _queue.addAll(node.takeShareRequests());
    if (_busy) return;
    _busy = true;
    try {
      while (_queue.isNotEmpty) {
        await _serve(_queue.removeAt(0));
      }
    } finally {
      _busy = false;
    }
  }

  Future<void> _serve(ShareRequestItem r) async {
    ShareFileInput? file;
    for (final s in node.ownShares()) {
      if (s.id != r.shareId) continue;
      for (final f in s.files) {
        if (f.id == r.itemId) file = f.file;
      }
    }
    if (file == null) return;
    final readable = await SharePicker.readable(r.handle);
    if (readable == null) return;
    try {
      await node.serveSharedItem(
        request: r,
        name: file.name,
        mediaType: file.mediaType,
        path: readable.path,
      );
    } on Object {
      // The share changed meanwhile or the file cannot be sent; the contact can ask again.
    } finally {
      if (readable.temporary) {
        try {
          await io.File(readable.path).delete();
        } on io.FileSystemException {
          // The cache is cleared by the system eventually.
        }
      }
    }
  }
}
