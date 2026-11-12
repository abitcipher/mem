import 'dart:io';

import 'package:material_ui/material_ui.dart';
import 'package:flutter/services.dart';
import 'package:share_plus/share_plus.dart';

import '../app_scope.dart';
import '../l10n/gen/app_localizations.dart';
import '../widgets/qr.dart';
import '../widgets/scan_or_paste.dart';
import 'chat.dart';
import 'receive.dart';
import 'sound_pair.dart';

/// Pairing: show my code, scan or paste theirs, or send a contact request to
/// someone out of reach.
class PairScreen extends StatelessWidget {
  const PairScreen({super.key});

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    // Pairing needs the identity key, which only the main device holds.
    if (!AppScope.of(context).node.isMainDevice()) {
      return Scaffold(
        appBar: AppBar(title: Text(l.pairTitle)),
        body: Padding(
          padding: const EdgeInsets.all(24),
          child: Text(l.addContactOnMain),
        ),
      );
    }
    return DefaultTabController(
      length: 3,
      child: Scaffold(
        appBar: AppBar(
          title: Text(l.pairTitle),
          bottom: TabBar(
            // Labels wrap to two lines rather than being cut off (longer languages, larger text).
            tabs: [
              for (final label in [l.myCodeTab, l.scanTab, l.requestTab])
                Tab(
                  height: 56,
                  child: Text(
                    label,
                    maxLines: 2,
                    textAlign: TextAlign.center,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
            ],
          ),
        ),
        body: TabBarView(
          children: [
            const _MyCode(),
            ScanOrPaste(
              onText: (text, channel) =>
                  receiveAndReport(context, text, channel),
              onScanned: (outcome) => reportOutcome(context, outcome),
            ),
            const _RequestForm(),
          ],
        ),
      ),
    );
  }
}

/// A contact request from a pasted code.
class _RequestForm extends StatefulWidget {
  const _RequestForm();

  @override
  State<_RequestForm> createState() => _RequestFormState();
}

class _RequestFormState extends State<_RequestForm> {
  final _code = TextEditingController();
  final _note = TextEditingController();
  bool _busy = false;

  @override
  void dispose() {
    _code.dispose();
    _note.dispose();
    super.dispose();
  }

  Future<void> _send() async {
    final l = AppLocalizations.of(context);
    final state = AppScope.read(context);
    setState(() => _busy = true);
    try {
      final note = _note.text.trim();
      final id = await state.node.sendContactRequest(
        cardText: _code.text,
        note: note.isEmpty ? null : note,
      );
      state.changed();
      if (!mounted) return;
      showSnack(context, l.requestPreparing);
      await Navigator.of(context).pushReplacement(
        MaterialPageRoute<void>(builder: (_) => ChatScreen(contactId: id)),
      );
    } on Object catch (e) {
      if (mounted) showSnack(context, reasonText(context, e));
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return ListView(
      padding: const EdgeInsets.all(24),
      children: [
        Text(l.requestHint),
        const SizedBox(height: 16),
        TextField(
          controller: _code,
          minLines: 2,
          maxLines: 4,
          decoration: InputDecoration(
            labelText: l.requestCodeLabel,
            border: const OutlineInputBorder(),
            suffixIcon: IconButton(
              tooltip: l.pasteFromClipboard,
              icon: const Icon(Icons.content_paste),
              onPressed: () async {
                final data = await Clipboard.getData(Clipboard.kTextPlain);
                if (data?.text != null) _code.text = data!.text!;
              },
            ),
          ),
        ),
        const SizedBox(height: 16),
        TextField(
          controller: _note,
          maxLines: 3,
          maxLength: 280,
          // The core counts bytes; this counts characters, so long non-Latin notes are
          // still checked there.
          decoration: InputDecoration(
            labelText: l.requestNoteLabel,
            helperText: l.requestNoteHelp,
            border: const OutlineInputBorder(),
          ),
        ),
        const SizedBox(height: 16),
        FilledButton.icon(
          icon: _busy
              ? const SizedBox.square(
                  dimension: 18,
                  child: CircularProgressIndicator(strokeWidth: 2),
                )
              : const Icon(Icons.person_add_alt),
          label: Text(l.sendRequest),
          onPressed: _busy ? null : _send,
        ),
      ],
    );
  }
}

class _MyCode extends StatelessWidget {
  const _MyCode();

  Future<bool> _confirmRemoteShare(BuildContext context) async {
    final l = AppLocalizations.of(context);
    return await showDialog<bool>(
          context: context,
          builder: (context) => AlertDialog(
            title: Text(l.consentTitle),
            content: Text(l.shareCodeWarning),
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
        ) ??
        false;
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final state = AppScope.of(context);
    final card = state.node.ownCard();
    final profile = state.node.profile();
    return ListView(
      padding: const EdgeInsets.all(24),
      children: [
        Text(l.myCodeHint, textAlign: TextAlign.center),
        const SizedBox(height: 16),
        if (card.qr != null)
          Center(
            child: ShowingCode(child: QrMatrix(qr: card.qr!)),
          ),
        const SizedBox(height: 16),
        if (profile.displayName != null)
          Text(
            profile.displayName!,
            textAlign: TextAlign.center,
            style: Theme.of(context).textTheme.titleMedium,
          ),
        Text(
          '${l.fingerprint}: ${profile.fingerprint}',
          textAlign: TextAlign.center,
          style: const TextStyle(fontFamily: 'monospace'),
        ),
        const SizedBox(height: 24),
        Wrap(
          alignment: WrapAlignment.center,
          spacing: 8,
          runSpacing: 8,
          children: [
            OutlinedButton.icon(
              icon: const Icon(Icons.copy),
              label: Text(l.copyCode),
              onPressed: () async {
                if (!await _confirmRemoteShare(context)) return;
                await Clipboard.setData(ClipboardData(text: card.text));
                if (context.mounted) showSnack(context, l.codeCopied);
              },
            ),
            if (Platform.isAndroid || Platform.isIOS)
              OutlinedButton.icon(
                icon: const Icon(Icons.share),
                label: Text(l.shareCode),
                onPressed: () async {
                  if (!await _confirmRemoteShare(context)) return;
                  await SharePlus.instance.share(ShareParams(text: card.text));
                },
              ),
            if (Platform.isAndroid)
              OutlinedButton.icon(
                icon: const Icon(Icons.graphic_eq),
                label: Text(l.soundPairButton),
                onPressed: () => Navigator.of(context).push(
                  MaterialPageRoute<void>(
                    builder: (_) => const SoundPairScreen(),
                  ),
                ),
              ),
            TextButton.icon(
              icon: const Icon(Icons.refresh),
              label: Text(l.newCode),
              onPressed: () async {
                await state.node.newPairingCode();
                state.changed();
              },
            ),
          ],
        ),
      ],
    );
  }
}
