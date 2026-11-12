import 'dart:async';
import 'dart:io';

import 'package:material_ui/material_ui.dart';
import 'package:flutter/services.dart';

import '../app_scope.dart';
import '../l10n/gen/app_localizations.dart';
import '../src/rust/api.dart';
import '../widgets/qr.dart';
import '../widgets/scan_or_paste.dart';

/// Confirming something of another service in person: scan the service's request,
/// read what it gives away, swap codes with the other phone, compare the emoji, sign, send.
///
/// Unlike everything else in MEM the result is not deniable, so this screen is its own entry
/// and always shows the disclosure first.
class AttestScreen extends StatefulWidget {
  const AttestScreen({super.key});

  @override
  State<AttestScreen> createState() => _AttestScreenState();
}

enum _Step { request, disclosure, exchange, compare, send }

class _AttestScreenState extends State<AttestScreen> {
  _Step _step = _Step.request;
  AttestFlow? _flow;
  String _emoji = '';
  SignedAttestation? _signed;
  bool _sending = false;
  bool? _sent;
  bool _copied = false;

  String _error(AppLocalizations l, Object e) => switch ('$e') {
    'wrong_object' => l.attestErrWrongObject,
    'other_subject' => l.attestErrOtherSubject,
    'same_role' => l.attestErrSameRole,
    'own_code' => l.attestErrOwnCode,
    'main_device_only' => l.attestMainOnly,
    _ => l.attestErrInvalid,
  };

  Future<void> _onRequest(String text) async {
    final l = AppLocalizations.of(context);
    final node = AppScope.read(context).node;
    try {
      final flow = await node.startAttestation(
        text: text,
        locale: Localizations.localeOf(context).languageCode,
      );
      if (!mounted) return;
      setState(() {
        _flow = flow;
        _step = _Step.disclosure;
      });
    } on Object catch (e) {
      if (mounted) showSnack(context, _error(l, e));
    }
  }

  Future<void> _onOffer(String text) async {
    final l = AppLocalizations.of(context);
    try {
      final emoji = _flow!.acceptPeer(text: text);
      HapticFeedback.mediumImpact();
      setState(() {
        _emoji = emoji;
        _step = _Step.compare;
      });
    } on Object catch (e) {
      showSnack(context, _error(l, e));
    }
  }

  void _cancel() {
    // Dropping the flow drops the derived key; nothing was signed.
    showSnack(context, AppLocalizations.of(context).attestCancelled);
    Navigator.of(context).pop();
  }

  Future<void> _confirm() async {
    final l = AppLocalizations.of(context);
    try {
      final signed = _flow!.sign();
      setState(() {
        _signed = signed;
        _step = _Step.send;
      });
      await _send();
    } on Object catch (e) {
      if (mounted) showSnack(context, _error(l, e));
    }
  }

  /// HTTPS POST of the attestation to the request's submit URL: 5 s, no
  /// automatic retries; only the status code is read.
  Future<void> _send() async {
    final signed = _signed!;
    final view = _flow!.request();
    final node = AppScope.read(context).node;
    setState(() {
      _sending = true;
      _sent = null;
    });
    final client = HttpClient()..connectionTimeout = const Duration(seconds: 5);
    var ok = false;
    try {
      final request = await client
          .postUrl(Uri.parse(signed.submitUrl))
          .timeout(const Duration(seconds: 5));
      request.followRedirects = false;
      request.headers.contentType = ContentType('application', 'cbor');
      request.add(signed.body);
      final response = await request.close().timeout(
        const Duration(seconds: 5),
      );
      await response.drain<void>();
      ok = response.statusCode >= 200 && response.statusCode < 300;
    } on Object {
      ok = false;
    } finally {
      client.close(force: true);
    }
    if (ok) {
      await node.recordAttestation(
        origin: view.origin,
        label: view.label,
        sent: true,
      );
    }
    if (!mounted) return;
    setState(() {
      _sending = false;
      _sent = ok;
    });
  }

  Future<void> _copy() async {
    final view = _flow!.request();
    await Clipboard.setData(ClipboardData(text: _signed!.text));
    if (!_copied) {
      _copied = true;
      if (!mounted) return;
      await AppScope.read(context).node.recordAttestation(
        origin: view.origin,
        label: view.label,
        sent: false,
      );
    }
    if (mounted) showSnack(context, AppLocalizations.of(context).codeCopied);
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final node = AppScope.of(context).node;
    return Scaffold(
      appBar: AppBar(title: Text(l.attestTitle)),
      body: !node.isMainDevice()
          ? Padding(
              padding: const EdgeInsets.all(24),
              child: Text(l.attestMainOnly),
            )
          : switch (_step) {
              _Step.request => _Scan(
                hint: l.attestScanRequest,
                onText: _onRequest,
              ),
              _Step.disclosure => _disclosure(l),
              _Step.exchange => _exchange(l),
              _Step.compare => _compare(l),
              _Step.send => _sendView(l),
            },
    );
  }

  Widget _disclosure(AppLocalizations l) {
    final r = _flow!.request();
    return ListView(
      padding: const EdgeInsets.all(24),
      children: [
        _row(l.attestAskedBy, r.origin),
        _row(l.attestWhat, r.label),
        _row(l.attestRole, r.role),
        const SizedBox(height: 8),
        Text(l.attestCheckCode(r.subjectShort)),
        const SizedBox(height: 16),
        Card(
          child: Padding(
            padding: const EdgeInsets.all(16),
            child: Text(r.disclosure, key: const Key('attest-disclosure')),
          ),
        ),
        const SizedBox(height: 16),
        Row(
          mainAxisAlignment: MainAxisAlignment.end,
          children: [
            TextButton(onPressed: _cancel, child: Text(l.cancel)),
            const SizedBox(width: 8),
            FilledButton(
              onPressed: () => setState(() => _step = _Step.exchange),
              child: Text(l.continueAction),
            ),
          ],
        ),
      ],
    );
  }

  Widget _exchange(AppLocalizations l) {
    final offer = _flow!.offer();
    return DefaultTabController(
      length: 2,
      child: Column(
        children: [
          TabBar(
            tabs: [
              Tab(text: l.myCodeTab),
              Tab(text: l.scanTab),
            ],
          ),
          Expanded(
            child: TabBarView(
              children: [
                ListView(
                  padding: const EdgeInsets.all(24),
                  children: [
                    Text(l.attestExchangeHint, textAlign: TextAlign.center),
                    const SizedBox(height: 16),
                    if (offer.qr != null)
                      Center(
                        child: ShowingCode(child: QrMatrix(qr: offer.qr!)),
                      ),
                    const SizedBox(height: 16),
                    Center(
                      child: OutlinedButton.icon(
                        icon: const Icon(Icons.copy),
                        label: Text(l.copyCode),
                        onPressed: () async {
                          await Clipboard.setData(
                            ClipboardData(text: offer.text),
                          );
                          if (mounted) showSnack(context, l.codeCopied);
                        },
                      ),
                    ),
                  ],
                ),
                _Scan(hint: l.attestExchangeHint, onText: _onOffer),
              ],
            ),
          ),
        ],
      ),
    );
  }

  Widget _compare(AppLocalizations l) => ListView(
    padding: const EdgeInsets.all(24),
    children: [
      Text(l.attestCompare),
      const SizedBox(height: 24),
      Text(
        _emoji,
        key: const Key('attest-emoji'),
        textAlign: TextAlign.center,
        style: const TextStyle(fontSize: 40),
      ),
      const SizedBox(height: 24),
      Row(
        mainAxisAlignment: MainAxisAlignment.end,
        children: [
          TextButton(onPressed: _cancel, child: Text(l.sasMismatch)),
          const SizedBox(width: 8),
          FilledButton(onPressed: _confirm, child: Text(l.sasMatch)),
        ],
      ),
    ],
  );

  Widget _sendView(AppLocalizations l) {
    final signed = _signed!;
    final origin = _flow!.request().origin;
    return ListView(
      padding: const EdgeInsets.all(24),
      children: [
        if (_sending) ...[
          const Center(child: CircularProgressIndicator()),
          const SizedBox(height: 16),
          Text(l.attestSending, textAlign: TextAlign.center),
        ] else if (_sent ?? false) ...[
          const Icon(Icons.verified_outlined, size: 56),
          const SizedBox(height: 16),
          Text(l.attestSent(origin), textAlign: TextAlign.center),
          const SizedBox(height: 16),
          Center(
            child: FilledButton(
              onPressed: () => Navigator.of(context).pop(),
              child: Text(l.done),
            ),
          ),
        ] else ...[
          Text(l.attestSendFailed),
          const SizedBox(height: 16),
          if (qrForText(text: signed.text) case final qr?)
            Center(
              child: ShowingCode(child: QrMatrix(qr: qr)),
            ),
          const SizedBox(height: 16),
          Wrap(
            alignment: WrapAlignment.end,
            spacing: 8,
            children: [
              OutlinedButton.icon(
                icon: const Icon(Icons.copy),
                label: Text(l.copyCode),
                onPressed: _copy,
              ),
              OutlinedButton(onPressed: _send, child: Text(l.attestTryAgain)),
              FilledButton(
                onPressed: () => Navigator.of(context).pop(),
                child: Text(l.done),
              ),
            ],
          ),
        ],
      ],
    );
  }

  Widget _row(String label, String value) => Padding(
    padding: const EdgeInsets.only(bottom: 8),
    child: Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        SizedBox(
          width: 120,
          child: Text(label, style: Theme.of(context).textTheme.labelLarge),
        ),
        Expanded(child: SelectableText(value)),
      ],
    ),
  );
}

/// Camera (where supported) plus a paste box, for attestation codes only.
class _Scan extends StatefulWidget {
  const _Scan({required this.hint, required this.onText});

  final String hint;
  final Future<void> Function(String text) onText;

  @override
  State<_Scan> createState() => _ScanState();
}

class _ScanState extends State<_Scan> {
  final _controller = TextEditingController();

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return ListView(
      padding: const EdgeInsets.all(16),
      children: [
        Text(widget.hint),
        const SizedBox(height: 16),
        if (ScanOrPaste.cameraSupported)
          CameraScanner(onScanned: (_) async {}, onAttest: widget.onText)
        else
          Text(l.cameraUnavailable),
        const SizedBox(height: 16),
        TextField(
          key: const Key('attest-paste'),
          controller: _controller,
          minLines: 2,
          maxLines: 4,
          decoration: InputDecoration(
            border: const OutlineInputBorder(),
            hintText: l.pasteHint,
          ),
        ),
        const SizedBox(height: 8),
        Row(
          children: [
            OutlinedButton.icon(
              icon: const Icon(Icons.content_paste),
              label: Text(l.pasteFromClipboard),
              onPressed: () async {
                final data = await Clipboard.getData(Clipboard.kTextPlain);
                if (data?.text != null) _controller.text = data!.text!;
              },
            ),
            const Spacer(),
            FilledButton(
              key: const Key('attest-open'),
              onPressed: () => widget.onText(_controller.text.trim()),
              child: Text(l.open),
            ),
          ],
        ),
      ],
    );
  }
}
