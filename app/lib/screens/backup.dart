import 'dart:convert';
import 'dart:math';
import 'dart:typed_data';

import 'package:material_ui/material_ui.dart';
import 'package:intl/intl.dart';
import 'package:share_plus/share_plus.dart';

import '../app_scope.dart';
import '../l10n/gen/app_localizations.dart';
import '../src/backup_schedule.dart';
import '../src/platform.dart';
import '../src/rust/api.dart';
import '../widgets/scan_or_paste.dart';
import 'send_flow.dart';

/// Backup and recovery: turn backups on, see the phrase, save or show a backup, and the
/// automatic schedule.
/// Shows a freshly drawn recovery phrase and asks for three of its words. Only a passed check
/// makes the phrase take effect; leaving earlier forgets it, so backups stay off
/// or keep the old phrase.
Future<void> setUpRecoveryPhrase(
  BuildContext context,
  PhraseFormItem form,
) async {
  final state = AppScope.read(context);
  final l = AppLocalizations.of(context);
  await state.node.newRecoveryPhrase(form: form);
  if (!context.mounted) return;
  final confirmed = await Navigator.of(context).push<bool>(
    MaterialPageRoute(builder: (_) => const PhraseScreen(confirm: true)),
  );
  if (confirmed ?? false) {
    await state.node.confirmRecoveryPhrase();
    if (context.mounted) showSnack(context, l.confirmDone);
  } else {
    await state.node.discardRecoveryPhrase();
    if (context.mounted) showSnack(context, l.phraseNotConfirmed);
  }
  state.changed();
}

class BackupScreen extends StatelessWidget {
  const BackupScreen({super.key});

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final state = AppScope.of(context);
    final status = state.node.backupStatus();
    return Scaffold(
      appBar: AppBar(title: Text(l.backupTitle)),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          Text(l.backupIntro),
          const SizedBox(height: 16),
          if (!status.enabled) const _TurnOn() else const _Actions(),
          const Divider(height: 40),
          ListTile(
            contentPadding: EdgeInsets.zero,
            leading: const Icon(Icons.restore),
            title: Text(l.restoreTitle),
            onTap: () => Navigator.of(context).push(
              MaterialPageRoute<void>(builder: (_) => const RestoreScreen()),
            ),
          ),
        ],
      ),
    );
  }
}

class _FormChoice extends StatelessWidget {
  const _FormChoice({required this.value, required this.onChanged});

  final PhraseFormItem value;
  final ValueChanged<PhraseFormItem> onChanged;

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return SegmentedButton<PhraseFormItem>(
      showSelectedIcon: false,
      segments: [
        ButtonSegment(
          value: PhraseFormItem.english,
          label: Text(l.formEnglish),
        ),
        ButtonSegment(
          value: PhraseFormItem.syllables,
          label: Text(l.formSyllables),
        ),
      ],
      selected: {value},
      onSelectionChanged: (s) => onChanged(s.first),
    );
  }
}

class _TurnOn extends StatefulWidget {
  const _TurnOn();

  @override
  State<_TurnOn> createState() => _TurnOnState();
}

class _TurnOnState extends State<_TurnOn> {
  PhraseFormItem? _form;

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    // Latin syllables by default for the Ukrainian UI, English words otherwise.
    final form =
        _form ??
        (Localizations.localeOf(context).languageCode == 'uk'
            ? PhraseFormItem.syllables
            : PhraseFormItem.english);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(l.backupOffBody),
        const SizedBox(height: 16),
        Text(
          l.phraseFormTitle,
          style: const TextStyle(fontWeight: FontWeight.bold),
        ),
        const SizedBox(height: 8),
        _FormChoice(value: form, onChanged: (f) => setState(() => _form = f)),
        const SizedBox(height: 16),
        FilledButton(
          onPressed: () => setUpRecoveryPhrase(context, form),
          child: Text(l.turnOnBackups),
        ),
      ],
    );
  }
}

class _Actions extends StatefulWidget {
  const _Actions();

  @override
  State<_Actions> createState() => _ActionsState();
}

class _ActionsState extends State<_Actions> {
  bool _history = true;
  bool _busy = false;

  Future<void> _run(Future<void> Function() task) async {
    setState(() => _busy = true);
    try {
      await task();
    } on Object catch (e) {
      if (mounted) showSnack(context, reasonText(context, e));
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final state = AppScope.of(context);
    final status = state.node.backupStatus();
    final node = state.node;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          title: Text(l.includeHistory),
          value: _history,
          onChanged: (v) => setState(() => _history = v),
        ),
        if (_busy) ...[
          const LinearProgressIndicator(),
          const SizedBox(height: 4),
          Text(l.working),
        ],
        ListTile(
          contentPadding: EdgeInsets.zero,
          enabled: !_busy,
          leading: const Icon(Icons.save_alt),
          title: Text(l.saveBackupFile),
          onTap: () => _run(() async {
            final bytes = await node.makeBackup(history: _history);
            final saved = await PlatformServices.saveFile(
              backupFileName(),
              bytes,
            );
            if (saved && context.mounted) showSnack(context, l.backupSaved);
          }),
        ),
        ListTile(
          contentPadding: EdgeInsets.zero,
          enabled: !_busy,
          leading: const Icon(Icons.qr_code_2),
          title: Text(l.showBackupQr),
          onTap: () => _run(() async {
            var animation = await node.backupAnimation(history: _history);
            if (animation == null && _history) {
              if (context.mounted) showSnack(context, l.backupTooLargeForQr);
              animation = await node.backupAnimation(history: false);
            }
            if (animation == null || !context.mounted) return;
            await Navigator.of(context).push(
              MaterialPageRoute<void>(
                fullscreenDialog: true,
                builder: (_) => AnimatedQrScreen(
                  animation: animation!,
                  title: l.showBackupQr,
                  hint: l.backupQrHint,
                ),
              ),
            );
          }),
        ),
        ListTile(
          contentPadding: EdgeInsets.zero,
          enabled: !_busy,
          leading: const Icon(Icons.share),
          title: Text(l.shareBackup),
          subtitle: Text(l.shareBackupWarning),
          onTap: () => _run(() async {
            final ok = await showDialog<bool>(
              context: context,
              builder: (context) => AlertDialog(
                title: Text(l.shareBackup),
                content: Text(l.shareBackupWarning),
                actions: [
                  TextButton(
                    onPressed: () => Navigator.pop(context, false),
                    child: Text(l.cancel),
                  ),
                  FilledButton(
                    onPressed: () => Navigator.pop(context, true),
                    child: Text(l.continueAction),
                  ),
                ],
              ),
            );
            if (ok != true) return;
            final bytes = await node.makeBackup(history: _history);
            await SharePlus.instance.share(
              ShareParams(
                files: [
                  XFile.fromData(
                    bytes,
                    name: backupFileName(),
                    mimeType: 'application/octet-stream',
                  ),
                ],
              ),
            );
          }),
        ),
        const Divider(height: 32),
        Text(
          l.phraseFormTitle,
          style: const TextStyle(fontWeight: FontWeight.bold),
        ),
        const SizedBox(height: 8),
        _FormChoice(
          value: status.form,
          onChanged: (f) async {
            await node.setPhraseForm(form: f);
            state.changed();
          },
        ),
        ListTile(
          contentPadding: EdgeInsets.zero,
          leading: const Icon(Icons.key),
          title: Text(l.showPhrase),
          onTap: () => Navigator.of(
            context,
          ).push(MaterialPageRoute<void>(builder: (_) => const PhraseScreen())),
        ),
        ListTile(
          contentPadding: EdgeInsets.zero,
          leading: const Icon(Icons.autorenew),
          title: Text(l.newPhrase),
          subtitle: Text(l.newPhraseWarning),
          onTap: () async {
            final ok = await showDialog<bool>(
              context: context,
              builder: (context) => AlertDialog(
                title: Text(l.newPhrase),
                content: Text(l.newPhraseWarning),
                actions: [
                  TextButton(
                    onPressed: () => Navigator.pop(context, false),
                    child: Text(l.cancel),
                  ),
                  FilledButton(
                    onPressed: () => Navigator.pop(context, true),
                    child: Text(l.continueAction),
                  ),
                ],
              ),
            );
            if (ok != true || !context.mounted) return;
            await setUpRecoveryPhrase(context, status.form);
          },
        ),
        const Divider(height: 32),
        const _Schedule(),
      ],
    );
  }
}

class _Schedule extends StatelessWidget {
  const _Schedule();

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final state = AppScope.of(context);
    final backups = state.backups;
    final s = backups.settings;
    final last = s.lastUnixMs;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          l.scheduleTitle,
          style: const TextStyle(fontWeight: FontWeight.bold),
        ),
        RadioGroup<BackupSchedule>(
          groupValue: s.schedule,
          onChanged: (v) {
            s.schedule = v ?? BackupSchedule.off;
            backups.save();
          },
          child: Column(
            children: [
              RadioListTile(
                contentPadding: EdgeInsets.zero,
                value: BackupSchedule.off,
                title: Text(l.scheduleOff),
              ),
              RadioListTile(
                contentPadding: EdgeInsets.zero,
                value: BackupSchedule.everyDays,
                title: Text(l.scheduleEveryDays(s.days)),
              ),
              RadioListTile(
                contentPadding: EdgeInsets.zero,
                value: BackupSchedule.afterNewContact,
                title: Text(l.scheduleAfterContact),
              ),
            ],
          ),
        ),
        if (s.schedule == BackupSchedule.everyDays)
          Slider(
            min: BackupSettings.minDays.toDouble(),
            max: BackupSettings.maxDays.toDouble(),
            divisions: BackupSettings.maxDays - BackupSettings.minDays,
            value: s.days.toDouble(),
            label: l.scheduleEveryDays(s.days),
            onChanged: (v) {
              s.days = v.round();
              backups.save();
            },
          ),
        if (s.schedule != BackupSchedule.off) ...[
          SwitchListTile(
            contentPadding: EdgeInsets.zero,
            title: Text(l.includeHistory),
            value: s.history,
            onChanged: (v) {
              s.history = v;
              backups.save();
            },
          ),
          ListTile(
            contentPadding: EdgeInsets.zero,
            leading: const Icon(Icons.folder_outlined),
            title: Text(l.backupFolder),
            subtitle: Text(
              s.folder == null ? l.noFolder : _folderLabel(s.folder!),
            ),
            trailing: TextButton(
              onPressed: () async {
                final folder = await PlatformServices.pickFolder();
                if (folder == null) return;
                s.folder = folder;
                backups.save();
                await backups.maybeRun();
              },
              child: Text(l.chooseFolder),
            ),
          ),
          Text(l.cloudFolderNote, style: Theme.of(context).textTheme.bodySmall),
        ],
        if (last != null) ...[
          const SizedBox(height: 8),
          Text(
            l.lastBackup(
              DateFormat.yMMMd(Localizations.localeOf(context).toString())
                  .add_Hm()
                  .format(DateTime.fromMillisecondsSinceEpoch(last)),
            ),
          ),
        ],
        if (s.lastFailed)
          Text(
            l.backupFailed,
            style: TextStyle(color: Theme.of(context).colorScheme.error),
          ),
      ],
    );
  }

  /// A readable end of an Android tree URI or a path.
  static String _folderLabel(String folder) {
    final decoded = Uri.decodeComponent(folder);
    final i = decoded.lastIndexOf(RegExp('[/:]'));
    return i >= 0 && i < decoded.length - 1
        ? decoded.substring(i + 1)
        : decoded;
  }
}

/// Shows the recovery phrase on a capture-protected screen. With `confirm`, asks
/// for three of the words afterwards.
class PhraseScreen extends StatefulWidget {
  const PhraseScreen({this.confirm = false, super.key});

  final bool confirm;

  @override
  State<PhraseScreen> createState() => _PhraseScreenState();
}

class _PhraseScreenState extends State<PhraseScreen> {
  List<String> _words = const [];

  @override
  void initState() {
    super.initState();
    PlatformServices.setSecure(on: true);
    AppScope.read(context).node.backupPhraseShow().then((w) {
      if (mounted) setState(() => _words = w);
    });
  }

  @override
  void dispose() {
    _words = const [];
    PlatformServices.setSecure(on: false);
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return Scaffold(
      appBar: AppBar(title: Text(l.phraseTitle)),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          Text(l.phraseHint),
          const SizedBox(height: 16),
          Wrap(
            spacing: 8,
            runSpacing: 8,
            children: [
              for (final (i, w) in _words.indexed)
                Chip(
                  label: Text(
                    '${i + 1}. $w',
                    style: const TextStyle(
                      fontFamily: 'monospace',
                      fontSize: 16,
                    ),
                  ),
                ),
            ],
          ),
          const SizedBox(height: 24),
          FilledButton(
            onPressed: _words.isEmpty
                ? null
                : () async {
                    if (!widget.confirm) {
                      Navigator.pop(context);
                      return;
                    }
                    // Back from the check returns here to look at the words again.
                    final passed = await Navigator.of(context).push<bool>(
                      MaterialPageRoute(
                        builder: (_) => _ConfirmPhraseScreen(words: _words),
                      ),
                    );
                    if ((passed ?? false) && context.mounted) {
                      Navigator.pop(context, true);
                    }
                  },
            child: Text(widget.confirm ? l.phraseWritten : l.done),
          ),
        ],
      ),
    );
  }
}

class _ConfirmPhraseScreen extends StatefulWidget {
  const _ConfirmPhraseScreen({required this.words});

  final List<String> words;

  @override
  State<_ConfirmPhraseScreen> createState() => _ConfirmPhraseScreenState();
}

class _ConfirmPhraseScreenState extends State<_ConfirmPhraseScreen> {
  late final List<int> _ask;
  late final List<TextEditingController> _fields;
  bool _wrong = false;

  @override
  void initState() {
    super.initState();
    PlatformServices.setSecure(on: true);
    final positions = List.generate(widget.words.length, (i) => i)
      ..shuffle(Random.secure());
    _ask = positions.take(3).toList()..sort();
    _fields = [for (final _ in _ask) TextEditingController()];
  }

  @override
  void dispose() {
    for (final f in _fields) {
      f
        ..clear()
        ..dispose();
    }
    PlatformServices.setSecure(on: false);
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return Scaffold(
      appBar: AppBar(title: Text(l.confirmTitle)),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          Text(l.confirmHint),
          const SizedBox(height: 16),
          for (final (i, pos) in _ask.indexed) ...[
            TextField(
              controller: _fields[i],
              autocorrect: false,
              enableSuggestions: false,
              enableIMEPersonalizedLearning: false,
              decoration: InputDecoration(labelText: l.wordN(pos + 1)),
            ),
            const SizedBox(height: 8),
          ],
          if (_wrong)
            Text(
              l.confirmWrong,
              style: TextStyle(color: Theme.of(context).colorScheme.error),
            ),
          const SizedBox(height: 16),
          FilledButton(
            onPressed: () {
              final ok = [
                for (final (i, pos) in _ask.indexed)
                  _fields[i].text.trim().toLowerCase() == widget.words[pos],
              ].every((x) => x);
              if (!ok) {
                setState(() => _wrong = true);
                return;
              }
              Navigator.pop(context, true);
            },
            child: Text(l.done),
          ),
        ],
      ),
    );
  }
}

/// Restore from a backup: scan the old phone's QR, open a file or paste text,
/// then type the phrase. Capture-protected while the phrase is on screen.
class RestoreScreen extends StatefulWidget {
  const RestoreScreen({this.initial, super.key});

  /// A backup already received (for example scanned elsewhere).
  final Uint8List? initial;

  @override
  State<RestoreScreen> createState() => _RestoreScreenState();
}

class _RestoreScreenState extends State<RestoreScreen> {
  Uint8List? _data;
  final _phrase = TextEditingController();
  final _paste = TextEditingController();
  bool _busy = false;
  String? _error;

  @override
  void initState() {
    super.initState();
    _data = widget.initial;
    PlatformServices.setSecure(on: true);
  }

  @override
  void dispose() {
    _phrase
      ..clear()
      ..dispose();
    _paste.dispose();
    PlatformServices.setSecure(on: false);
    super.dispose();
  }

  Future<void> _restore() async {
    final l = AppLocalizations.of(context);
    final state = AppScope.read(context);
    final data = _data;
    if (data == null) return;
    if (state.node.contacts().isNotEmpty) {
      final ok = await showDialog<bool>(
        context: context,
        builder: (context) => AlertDialog(
          title: Text(l.restoreReplaceTitle),
          content: Text(l.restoreReplaceBody),
          actions: [
            TextButton(
              onPressed: () => Navigator.pop(context, false),
              child: Text(l.cancel),
            ),
            FilledButton(
              onPressed: () => Navigator.pop(context, true),
              child: Text(l.restoreAction),
            ),
          ],
        ),
      );
      if (ok != true) return;
    }
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      final result = await state.node.restoreWithPhrase(
        data: data,
        words: _phrase.text,
      );
      state.changed();
      if (!mounted) return;
      await showDialog<void>(
        context: context,
        builder: (context) => AlertDialog(
          title: Text(l.restoreTitle),
          content: Text(
            [
              if (result.correctedPhrase != null)
                l.restoreCorrected(result.correctedPhrase!),
              l.restoreDone,
            ].join('\n\n'),
          ),
          actions: [
            FilledButton(
              onPressed: () => Navigator.pop(context),
              child: Text(l.done),
            ),
          ],
        ),
      );
      if (mounted) Navigator.of(context).popUntil((r) => r.isFirst);
    } on Object catch (e) {
      final text = e.toString();
      setState(
        () => _error = text.contains('not a MEM backup')
            ? l.restoreNotBackup
            : l.restoreWrongPhrase,
      );
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final data = _data;
    return Scaffold(
      appBar: AppBar(title: Text(l.restoreTitle)),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          if (data == null) ...[
            Text(l.restoreHint),
            const SizedBox(height: 16),
            CameraScanner(
              onScanned: (_) async => showSnack(context, l.restoreNotBackup),
              onBackup: (bytes) async => setState(() => _data = bytes),
            ),
            const SizedBox(height: 16),
            OutlinedButton.icon(
              icon: const Icon(Icons.file_open_outlined),
              label: Text(l.openBackupFile),
              onPressed: () async {
                final bytes = await PlatformServices.openFile();
                if (bytes != null) setState(() => _data = bytes);
              },
            ),
            const SizedBox(height: 16),
            TextField(
              controller: _paste,
              minLines: 2,
              maxLines: 4,
              decoration: InputDecoration(labelText: l.pasteBackup),
              onChanged: (t) {
                if (t.trim().startsWith('membackup1:')) {
                  setState(
                    () => _data = Uint8List.fromList(utf8.encode(t.trim())),
                  );
                }
              },
            ),
          ] else ...[
            Text(l.backupReceived),
            const SizedBox(height: 16),
            TextField(
              controller: _phrase,
              autocorrect: false,
              enableSuggestions: false,
              enableIMEPersonalizedLearning: false,
              minLines: 2,
              maxLines: 4,
              decoration: InputDecoration(labelText: l.phraseInput),
            ),
            if (_error != null) ...[
              const SizedBox(height: 8),
              Text(
                _error!,
                style: TextStyle(color: Theme.of(context).colorScheme.error),
              ),
            ],
            const SizedBox(height: 16),
            if (_busy) ...[
              const LinearProgressIndicator(),
              const SizedBox(height: 4),
              Text(l.working),
            ],
            FilledButton(
              onPressed: _busy ? null : _restore,
              child: Text(l.restoreAction),
            ),
          ],
        ],
      ),
    );
  }
}
