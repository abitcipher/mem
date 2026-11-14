import 'dart:convert';

import 'package:material_ui/material_ui.dart';

import '../app_scope.dart';
import '../l10n/gen/app_localizations.dart';
import '../src/rust/api.dart';
import '../src/shares.dart';
import '../widgets/attachments.dart';

/// Our shares for one contact or one group: add files or a folder, see who downloaded what,
/// stop sharing. Exactly one of [contactId] and [groupId] is given.
class SharesScreen extends StatelessWidget {
  const SharesScreen({this.contactId, this.groupId, super.key})
    : assert((contactId == null) != (groupId == null), 'one audience');

  final String? contactId;
  final String? groupId;

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final state = AppScope.of(context);
    final shares = state.node
        .ownShares()
        .where((s) => s.contactId == contactId && s.groupId == groupId)
        .toList();
    final names = {
      for (final c in state.node.contacts()) c.id: contactTitle(c),
    };
    return Scaffold(
      appBar: AppBar(title: Text(l.sharesTitle)),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          Text(l.sharesHelp, style: Theme.of(context).textTheme.bodySmall),
          const SizedBox(height: 12),
          Wrap(
            spacing: 8,
            runSpacing: 8,
            children: [
              FilledButton.tonalIcon(
                icon: const Icon(Icons.note_add_outlined),
                label: Text(l.shareAddFiles),
                onPressed: () => _addFiles(context, null),
              ),
              FilledButton.tonalIcon(
                icon: const Icon(Icons.create_new_folder_outlined),
                label: Text(l.shareAddFolder),
                onPressed: () => _addFolder(context),
              ),
            ],
          ),
          const SizedBox(height: 16),
          if (shares.isEmpty) Text(l.shareNone),
          for (final s in shares)
            _OwnShareCard(
              share: s,
              names: names,
              onAddFiles: () => _addFiles(context, s),
              onSave: (name, files, introduced) => _save(
                context,
                share: s,
                name: name,
                files: files,
                introduced: introduced,
              ),
              onStop: () => _stop(context, s),
            ),
        ],
      ),
    );
  }

  Future<void> _addFiles(BuildContext context, OwnShareItem? to) async {
    final l = AppLocalizations.of(context);
    final picked = await SharePicker.pickFiles();
    if (picked == null || picked.isEmpty || !context.mounted) return;
    if (to != null) {
      await _save(
        context,
        share: to,
        name: to.name,
        files: [...to.files.map((f) => f.file), ...picked],
        introduced: to.includeIntroduced,
      );
      return;
    }
    final name = await _askName(context, l.sharesTitle);
    if (name == null || !context.mounted) return;
    await _save(context, name: name, files: picked);
  }

  Future<void> _addFolder(BuildContext context) async {
    final folder = await SharePicker.pickFolder();
    if (folder == null || !context.mounted) return;
    final name = await _askName(context, folder.name);
    if (name == null || !context.mounted) return;
    await _save(context, name: name, files: folder.files);
  }

  Future<void> _save(
    BuildContext context, {
    required String name,
    required List<ShareFileInput> files,
    OwnShareItem? share,
    bool introduced = false,
  }) async {
    final l = AppLocalizations.of(context);
    final state = AppScope.read(context);
    // One entry per file; files above the attachment limit cannot be sent.
    final seen = <String>{};
    final kept = <ShareFileInput>[];
    var large = 0;
    for (final f in files) {
      if (!seen.add(f.handle)) continue;
      if (f.size > BigInt.from(maxSharedFile)) {
        large++;
        continue;
      }
      if (kept.length < maxShareFiles) kept.add(f);
    }
    try {
      if (kept.isEmpty && share != null) {
        await state.node.removeShare(shareId: share.id);
      } else if (kept.isNotEmpty) {
        await state.node.saveShare(
          shareId: share?.id,
          name: fitName(name),
          contactId: contactId,
          groupId: groupId,
          includeIntroduced: introduced,
          files: kept,
        );
      }
      state.changed();
      if (large > 0 && context.mounted) {
        showSnack(context, l.shareSkippedLarge(large));
      }
    } on Object catch (e) {
      if (context.mounted) showSnack(context, reasonText(context, e));
    }
  }

  Future<void> _stop(BuildContext context, OwnShareItem s) async {
    final l = AppLocalizations.of(context);
    final state = AppScope.read(context);
    final ok = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(l.shareStop),
        content: Text(l.shareStopConfirm),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context, false),
            child: Text(l.cancel),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(context, true),
            child: Text(l.shareStop),
          ),
        ],
      ),
    );
    if (ok != true || !context.mounted) return;
    try {
      await state.node.removeShare(shareId: s.id);
      state.changed();
    } on Object catch (e) {
      if (context.mounted) showSnack(context, reasonText(context, e));
    }
  }
}

/// A share name of at most 64 bytes, cut at a character.
String fitName(String name) {
  var n = name.trim().isEmpty ? 'Files' : name.trim();
  while (utf8.encode(n).length > 64) {
    n = n.characters.skipLast(1).toString();
  }
  return n;
}

/// Asks for a share's name, starting from `initial`.
Future<String?> _askName(BuildContext context, String initial) async {
  final l = AppLocalizations.of(context);
  // The suggestion is selected, so typing replaces it.
  final controller = TextEditingController(text: initial)
    ..selection = TextSelection(baseOffset: 0, extentOffset: initial.length);
  final name = await showDialog<String>(
    context: context,
    builder: (context) => AlertDialog(
      title: Text(l.shareNewName),
      content: TextField(
        controller: controller,
        autofocus: true,
        maxLength: 64,
        decoration: InputDecoration(helperText: l.shareNewNameHelp),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.pop(context),
          child: Text(l.cancel),
        ),
        FilledButton(
          onPressed: () => Navigator.pop(context, controller.text),
          child: Text(l.save),
        ),
      ],
    ),
  );
  controller.dispose();
  return name;
}

class _OwnShareCard extends StatelessWidget {
  const _OwnShareCard({
    required this.share,
    required this.names,
    required this.onAddFiles,
    required this.onSave,
    required this.onStop,
  });

  final OwnShareItem share;
  final Map<String, String> names;
  final VoidCallback onAddFiles;
  final void Function(String name, List<ShareFileInput> files, bool introduced)
  onSave;
  final VoidCallback onStop;

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final files = share.files.map((f) => f.file).toList();
    return Card(
      margin: const EdgeInsets.only(bottom: 12),
      child: ExpansionTile(
        leading: const Icon(Icons.folder_shared_outlined),
        title: Text(share.name),
        subtitle: Text(l.shareFiles(share.files.length)),
        trailing: PopupMenuButton<String>(
          itemBuilder: (_) => [
            PopupMenuItem(value: 'add', child: Text(l.shareAddFiles)),
            PopupMenuItem(value: 'rename', child: Text(l.shareRename)),
            PopupMenuItem(value: 'stop', child: Text(l.shareStop)),
          ],
          onSelected: (v) async {
            switch (v) {
              case 'add':
                onAddFiles();
              case 'rename':
                final name = await _askName(context, share.name);
                if (name != null) onSave(name, files, share.includeIntroduced);
              case 'stop':
                onStop();
            }
          },
        ),
        children: [
          if (share.groupId != null)
            SwitchListTile(
              title: Text(l.shareIntroduced),
              subtitle: Text(l.shareIntroducedHelp),
              value: share.includeIntroduced,
              onChanged: (v) => onSave(share.name, files, v),
            ),
          for (final f in share.files)
            ListTile(
              dense: true,
              leading: const Icon(Icons.insert_drive_file_outlined),
              title: Text(f.file.name),
              subtitle: Text(
                [
                  formatSize(f.file.size.toInt()),
                  if (f.downloads.isEmpty)
                    l.shareNotDownloaded
                  else
                    l.shareDownloadedBy(
                      f.downloads
                          .map(
                            (d) =>
                                names[d.contactId] ??
                                d.contactId.substring(0, 8),
                          )
                          .toSet()
                          .join(', '),
                    ),
                ].join(' · '),
              ),
              trailing: IconButton(
                tooltip: l.shareRemoveFile,
                icon: const Icon(Icons.remove_circle_outline),
                onPressed: () => onSave(share.name, [
                  for (final o in share.files)
                    if (o.id != f.id) o.file,
                ], share.includeIntroduced),
              ),
            ),
        ],
      ),
    );
  }
}

/// Files contacts offer us: from one contact ([contactId]), in one group ([groupId]), or all.
class SharedWithMeScreen extends StatelessWidget {
  const SharedWithMeScreen({this.contactId, this.groupId, super.key});

  final String? contactId;
  final String? groupId;

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final state = AppScope.of(context);
    final shares = state.node
        .sharedWithMe(contactId: contactId)
        .where((s) => groupId == null || s.groupId == groupId)
        .toList();
    final names = {
      for (final c in state.node.contacts()) c.id: contactTitle(c),
    };
    final groups = {for (final g in state.node.groups()) g.id: g.name};
    return Scaffold(
      appBar: AppBar(title: Text(l.sharedWithMe)),
      body: shares.isEmpty
          ? Center(
              child: Padding(
                padding: const EdgeInsets.all(24),
                child: Text(l.sharedNone),
              ),
            )
          : ListView(
              padding: const EdgeInsets.all(16),
              children: [
                for (final s in shares)
                  Card(
                    margin: const EdgeInsets.only(bottom: 12),
                    child: ExpansionTile(
                      initiallyExpanded: shares.length == 1,
                      leading: const Icon(Icons.folder_outlined),
                      title: Text(s.name),
                      subtitle: Text(
                        [
                          l.sharedFrom(
                            names[s.contactId] ?? s.contactId.substring(0, 8),
                          ),
                          if (s.groupId case final g?)
                            l.sharedInGroup(groups[g] ?? g.substring(0, 8)),
                          l.shareFiles(s.files.length),
                        ].join(' · '),
                      ),
                      children: [
                        for (final f in s.files)
                          _TheirFileTile(share: s, file: f),
                      ],
                    ),
                  ),
              ],
            ),
    );
  }
}

class _TheirFileTile extends StatelessWidget {
  const _TheirFileTile({required this.share, required this.file});

  final TheirShareItem share;
  final TheirShareFileItem file;

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final state = AppScope.read(context);
    final size = formatSize(file.size.toInt());
    return switch (file.state) {
      SharedFileState_Available() => ListTile(
        dense: true,
        leading: const Icon(Icons.insert_drive_file_outlined),
        title: Text(file.name),
        subtitle: Text(size),
        trailing: TextButton(
          child: Text(l.shareDownload),
          onPressed: () async {
            try {
              await state.node.requestSharedItem(
                contactId: share.contactId,
                shareId: share.id,
                itemId: file.id,
              );
              state.changed();
            } on Object catch (e) {
              if (context.mounted) showSnack(context, reasonText(context, e));
            }
          },
        ),
      ),
      SharedFileState_Requested() => ListTile(
        dense: true,
        leading: const Icon(Icons.downloading_outlined),
        title: Text(file.name),
        subtitle: Text('$size · ${l.shareRequested}'),
      ),
      SharedFileState_Received(:final messageId) => ListTile(
        dense: true,
        leading: const Icon(Icons.download_done_outlined),
        title: Text(file.name),
        subtitle: Text(size),
        trailing: TextButton(
          child: Text(l.shareOpen),
          onPressed: () => exportAttachment(
            context,
            messageId,
            0,
            AttachmentItem(
              name: file.name,
              mediaType: file.mediaType,
              size: file.size,
              preview: null,
              state: const AttachmentStatus.ready(),
            ),
          ),
        ),
      ),
    };
  }
}

/// The quiet line in a chat when a contact shares new files; opens what they share.
class ShareNoticeLine extends StatelessWidget {
  const ShareNoticeLine({
    required this.name,
    required this.count,
    required this.contactId,
    this.groupId,
    super.key,
  });

  final String name;
  final int count;
  final String contactId;
  final String? groupId;

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return Center(
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: 6),
        child: ActionChip(
          avatar: const Icon(Icons.folder_shared_outlined, size: 18),
          label: Text(l.shareNotice(name, count)),
          onPressed: () => Navigator.of(context).push(
            MaterialPageRoute<void>(
              builder: (_) => SharedWithMeScreen(
                contactId: groupId == null ? contactId : null,
                groupId: groupId,
              ),
            ),
          ),
        ),
      ),
    );
  }
}
