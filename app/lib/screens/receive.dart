import 'package:material_ui/material_ui.dart';

import '../app_scope.dart';
import '../l10n/gen/app_localizations.dart';
import '../src/rust/api.dart';
import '../theme.dart';
import '../widgets/scan_or_paste.dart';
import 'chat.dart';
import 'requests.dart';

/// Scan or paste anything MEM: a contact's code or a message.
class ReceiveScreen extends StatelessWidget {
  const ReceiveScreen({super.key});

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return Scaffold(
      appBar: AppBar(title: Text(l.receive)),
      body: ScanOrPaste(
        onText: (text, channel) => receiveAndReport(context, text, channel),
        onScanned: (outcome) => reportOutcome(context, outcome),
      ),
    );
  }
}

/// Feeds text to the node and shows the result: opens the chat for a message, shows the
/// short authentication string for a new contact, or explains a rejection.
Future<void> receiveAndReport(
  BuildContext context,
  String text,
  Channel channel,
) async {
  final state = AppScope.read(context);
  try {
    final outcome = await state.node.receiveText(text: text, channel: channel);
    state.changed();
    if (context.mounted) await reportOutcome(context, outcome);
  } on Object catch (e) {
    if (context.mounted) showSnack(context, reasonText(context, e));
  }
}

/// Shows what a received item turned out to be.
Future<void> reportOutcome(BuildContext context, ReceiveOutcome outcome) async {
  final l = AppLocalizations.of(context);
  switch (outcome) {
    case ReceiveOutcome_Message(:final contactId):
      showSnack(context, l.messageReceived);
      // Land on exactly one chat with this contact, above the conversation list.
      final navigator = Navigator.of(context)
        ..popUntil((route) => route.isFirst);
      await navigator.push(
        MaterialPageRoute<void>(
          builder: (_) => ChatScreen(contactId: contactId),
        ),
      );
    // The same card again: nothing new to check (the comparison is for new keys). The
    // symbols stay one tap away as an optional extra check.
    case ReceiveOutcome_Contact(unchanged: true, :final contactId, :final sas):
      showSnack(
        context,
        l.alreadyContact,
        icon: Icons.verified_user_outlined,
        iconColor: MemColors.protected(context),
        action: SnackBarAction(
          label: l.compareSymbols,
          // The screen that scanned may be gone by now; then the button only hides the bar.
          onPressed: () {
            if (!context.mounted) return;
            showSasDialog(
              context,
              contactId: contactId,
              updated: true,
              sas: sas,
              title: l.compareSymbols,
            );
          },
        ),
      );
    case ReceiveOutcome_Contact(:final contactId, :final updated, :final sas):
      await showSasDialog(
        context,
        contactId: contactId,
        updated: updated,
        sas: sas,
      );
    case ReceiveOutcome_Request():
      showSnack(context, l.requestReceived);
      final navigator = Navigator.of(context)
        ..popUntil((route) => route.isFirst);
      await navigator.push(
        MaterialPageRoute<void>(builder: (_) => const RequestsScreen()),
      );
    case ReceiveOutcome_Rejected(:final reason):
      showSnack(context, reasonText(context, reason));
    case ReceiveOutcome_NotMem():
      showSnack(context, l.notMem);
  }
}

/// Both users compare the same symbols.
Future<void> showSasDialog(
  BuildContext context, {
  required String contactId,
  required bool updated,
  required String sas,
  String? title,
}) async {
  final l = AppLocalizations.of(context);
  final match = await showDialog<bool>(
    context: context,
    barrierDismissible: false,
    builder: (context) => AlertDialog(
      title: Text(
        title ?? (updated ? l.contactUpdatedTitle : l.contactAddedTitle),
      ),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(l.compareSas),
          const SizedBox(height: 20),
          Text(
            sas,
            style: const TextStyle(fontSize: 34),
            textAlign: TextAlign.center,
          ),
        ],
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.pop(context, false),
          child: Text(l.sasMismatch),
        ),
        FilledButton(
          onPressed: () => Navigator.pop(context, true),
          child: Text(l.sasMatch),
        ),
      ],
    ),
  );
  if (!context.mounted) return;
  if (match == false) {
    await showDialog<void>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(l.sasMismatch),
        content: Text(l.sasMismatchBody),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context),
            child: Text(l.done),
          ),
        ],
      ),
    );
  }
}
