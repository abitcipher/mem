import 'package:material_ui/material_ui.dart';

import '../app_scope.dart';
import '../l10n/gen/app_localizations.dart';
import '../screens/receive.dart';
import '../src/platform.dart';
import '../src/rust/api.dart';

/// Opens MEM text that another app handed over: a message or a contact's code shared
/// to MEM, or chosen in the menu of selected text. It goes through the same path as pasting on
/// the Receive screen, recorded as the Share channel (P4). MEM never reads the clipboard itself.
///
/// Any app can hand text to MEM without the user doing anything, so a new contact code is only
/// imported after the user confirms it (contacts are approved, never added by another app).
class InboundText extends StatefulWidget {
  const InboundText({required this.child, super.key});

  final Widget child;

  @override
  State<InboundText> createState() => _InboundTextState();
}

class _InboundTextState extends State<InboundText> with WidgetsBindingObserver {
  var _busy = false;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    PlatformServices.onInboundText(_take);
    WidgetsBinding.instance.addPostFrameCallback((_) => _take());
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    super.dispose();
  }

  // An activity started by a share while the engine kept running gets the text in onCreate,
  // without a call into Dart: look again whenever MEM comes to the front.
  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.resumed) _take();
  }

  Future<void> _take() async {
    if (_busy) return;
    _busy = true;
    try {
      final text = (await PlatformServices.takeInboundText())?.trim();
      if (text == null || !mounted) return;
      final card = AppScope.read(context).node.cardPreview(text: text);
      if (card != null && !card.known && !await _confirm(card)) return;
      if (mounted) await receiveAndReport(context, text, Channel.share);
    } finally {
      _busy = false;
    }
  }

  /// Asks before adding a contact whose code another app handed over.
  Future<bool> _confirm(CardPreviewItem card) async {
    final l = AppLocalizations.of(context);
    final ok = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(l.inboundCardTitle),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(l.inboundCardBody),
            const SizedBox(height: 12),
            if (card.displayName != null)
              Text(
                card.displayName!,
                style: Theme.of(context).textTheme.titleMedium,
              ),
            SelectableText(
              card.fingerprint,
              style: const TextStyle(fontFamily: 'monospace'),
            ),
          ],
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context, false),
            child: Text(l.cancel),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(context, true),
            child: Text(l.addContact),
          ),
        ],
      ),
    );
    return ok ?? false;
  }

  @override
  Widget build(BuildContext context) => widget.child;
}
