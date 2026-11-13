import 'package:material_ui/material_ui.dart';
import 'package:flutter_chat_core/flutter_chat_core.dart' as fc;
import 'package:flutter_chat_ui/flutter_chat_ui.dart';

import '../app_scope.dart';
import '../l10n/gen/app_localizations.dart';
import '../src/rust/api.dart';
import '../calls/call_line.dart';
import '../widgets/attachments.dart';
import '../widgets/chat_sync.dart';
import '../widgets/message_bubble.dart';
import 'contact.dart';
import 'receive.dart';
import 'send_flow.dart';

const _me = 'me';

class ChatScreen extends StatefulWidget {
  const ChatScreen({required this.contactId, super.key});

  final String contactId;

  @override
  State<ChatScreen> createState() => _ChatScreenState();
}

class _ChatScreenState extends State<ChatScreen> {
  final _controller = fc.InMemoryChatController();
  Map<String, MessageItem> _items = {};

  /// The contact's privacy floor, for which symbols the bubbles keep.
  int? _floor;
  AppState? _state;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final state = AppScope.of(context);
    if (_state != state) {
      _state?.removeListener(_sync);
      _state = state..addListener(_sync);
    }
    _sync();
  }

  @override
  void dispose() {
    _state?.removeListener(_sync);
    _controller.dispose();
    super.dispose();
  }

  String _noNote = '';

  void _sync() {
    if (mounted) _noNote = AppLocalizations.of(context).requestNoNote;
    final items = _state!.node.messages(contactId: widget.contactId);
    _items = {for (final m in items) m.id: m};
    _floor = _state!.node
        .contacts()
        .where((c) => c.id == widget.contactId)
        .firstOrNull
        ?.floor;
    final next = [
      for (final m in items)
        fc.Message.text(
          id: m.id,
          authorId: m.outgoing ? _me : widget.contactId,
          createdAt: DateTime.fromMillisecondsSinceEpoch(
            m.createdUnixMs.toInt(),
          ),
          text: m.isRequest && m.text.isEmpty ? _noNote : m.text,
          // No spinner for queued messages: nothing is in progress until the user picks a
          // channel; the provenance line under the bubble says "tap to send" instead.
          status: switch (m.status) {
            MessageStatus.sent => fc.MessageStatus.sent,
            MessageStatus.delivered => fc.MessageStatus.delivered,
            MessageStatus.failed => fc.MessageStatus.error,
            MessageStatus.queued || MessageStatus.received => null,
          },
        ),
    ];
    showMessages(_controller, next);
    if (mounted) setState(() {});
  }

  /// Corrects one of our messages: the new text replaces it here and at the contact.
  Future<void> _edit(BuildContext context, MessageItem item) async {
    final l = AppLocalizations.of(context);
    final controller = TextEditingController(text: item.text);
    final text = await showDialog<String>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(l.editMessage),
        content: TextField(
          controller: controller,
          autofocus: true,
          maxLines: null,
          maxLength: 4000,
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
    final next = text?.trim() ?? '';
    if (next.isEmpty || next == item.text || !context.mounted) return;
    final state = AppScope.read(context);
    try {
      await state.node.editMessage(messageId: item.id, text: next);
      state.changed();
    } on Object catch (e) {
      if (context.mounted) showSnack(context, reasonText(context, e));
    }
  }

  Future<void> _send(String text) async {
    final state = AppScope.read(context);
    try {
      final sent = await state.node.sendText(
        contactId: widget.contactId,
        text: text,
      );
      state.changed();
      // Already on its way over Wi-Fi or through the hub: no need to ask how it should
      // travel. Tapping the message still offers every channel.
      if (mounted && !sent.sendingNearby && !sent.sendingHub) {
        await startSendFlow(context, sent.messageId);
      }
    } on Object catch (e) {
      if (mounted) showSnack(context, reasonText(context, e));
    }
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final contact = _state!.node
        .contacts()
        .where((c) => c.id == widget.contactId)
        .firstOrNull;
    final title = contact == null ? '' : contactTitle(contact);
    final nearby = _state!.node.nearby().contains(widget.contactId);
    return Scaffold(
      appBar: AppBar(
        title: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(title),
            if (nearby)
              Row(
                children: [
                  const Icon(Icons.near_me, size: 14),
                  const SizedBox(width: 4),
                  Text(l.nearby, style: Theme.of(context).textTheme.labelSmall),
                ],
              ),
          ],
        ),
        actions: [
          if (_state!.calls.enabled)
            IconButton(
              tooltip: l.callStart,
              icon: const Icon(Icons.call_outlined),
              onPressed: () async {
                final error = await _state!.calls.start(widget.contactId);
                if (error != null && context.mounted) {
                  showSnack(context, reasonText(context, error));
                }
              },
            ),
          IconButton(
            tooltip: l.receive,
            icon: const Icon(Icons.qr_code_scanner),
            onPressed: () => Navigator.of(context).push(
              MaterialPageRoute<void>(builder: (_) => const ReceiveScreen()),
            ),
          ),
          IconButton(
            tooltip: l.contactSettings,
            icon: const Icon(Icons.info_outline),
            onPressed: () => Navigator.of(context).push(
              MaterialPageRoute<void>(
                builder: (_) => ContactScreen(contactId: widget.contactId),
              ),
            ),
          ),
        ],
      ),
      body: Column(
        children: [
          if (contact?.requested ?? false)
            MaterialBanner(
              leading: const Icon(Icons.hourglass_top),
              content: Text(l.requestPendingBanner(title)),
              actions: const [SizedBox.shrink()],
            ),
          Expanded(child: _chat(context, title)),
        ],
      ),
    );
  }

  // A transparent Material ancestor for the chat's composer and its ink effects.
  Widget _chat(BuildContext context, String title) => Material(
    type: MaterialType.transparency,
    child: Chat(
      currentUserId: _me,
      theme: chatTheme(Theme.of(context)),
      chatController: _controller,
      resolveUser: (id) async =>
          fc.User(id: id, name: id == _me ? null : title),
      onMessageSend: _send,
      // Small files only in this version; groups do not take files yet.
      onAttachmentTap: () =>
          pickAndSendFiles(context, contactId: widget.contactId),
      onMessageLongPress:
          (context, message, {required index, required details}) {
            final item = _items[message.id];
            // Only our own texts, while the recipient still accepts edits.
            if (item != null && item.editable) _edit(context, item);
          },
      onMessageTap: (context, message, {required index, required details}) {
        final item = _items[message.id];
        // Unconfirmed messages can be sent again by any channel (one-tap resend).
        if (item != null &&
            item.outgoing &&
            item.status != MessageStatus.delivered) {
          startSendFlow(context, item.id);
        }
      },
      builders: fc.Builders(
        composerBuilder: (context) =>
            Composer(hintText: AppLocalizations.of(context).typeMessage),
        textMessageBuilder:
            (context, message, index, {required isSentByMe, groupStatus}) {
              final item = _items[message.id];
              final settings = AppScope.read(context).settings;
              final call = item?.call;
              if (item != null && call != null) {
                return CallLine(item: item, call: call);
              }
              return Column(
                crossAxisAlignment: isSentByMe
                    ? CrossAxisAlignment.end
                    : CrossAxisAlignment.start,
                children: [
                  if (item?.isRequest ?? false)
                    Padding(
                      padding: const EdgeInsets.only(bottom: 2),
                      child: Text(
                        AppLocalizations.of(context).contactRequestLabel,
                        style: Theme.of(context).textTheme.labelSmall,
                      ),
                    ),
                  MessageBubble(
                    message: message,
                    isSentByMe: isSentByMe,
                    item: item,
                    settings: settings,
                    floor: _floor,
                  ),
                  if (item != null) Provenance(item: item),
                ],
              );
            },
      ),
    ),
  );
}

/// The lines under a bubble that ask for action or report group progress. Which channels
/// carried the message is shown inside the bubble ([Indication]).
class Provenance extends StatelessWidget {
  const Provenance({required this.item, super.key});

  final MessageItem item;

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final muted = TextStyle(
      fontSize: 11,
      color: Theme.of(context).colorScheme.onSurfaceVariant,
    );
    // Our group message: copies go out by themselves; show how many confirmed.
    final total = item.sentTo;
    final lines = <Widget>[
      if (item.channels.isEmpty && total != null)
        Text(l.groupSentAutomatically, style: muted),
      if (item.channels.isEmpty && total == null)
        Text(
          l.tapToSend,
          style: TextStyle(
            fontSize: 11,
            color: Theme.of(context).colorScheme.error,
          ),
        ),
      if (total != null)
        Text(l.deliveredToCount(item.deliveredTo ?? 0, total), style: muted),
      // Sent but not confirmed: Wi-Fi, Bluetooth and the hub carry on by themselves; QR and
      // share need the user.
      if (total == null &&
          item.channels.isNotEmpty &&
          item.outgoing &&
          item.status == MessageStatus.sent)
        Text(
          item.channels.any(
                (u) =>
                    u.channel == Channel.lan ||
                    u.channel == Channel.ble ||
                    u.channel == Channel.wifiDirect ||
                    u.channel == Channel.hub,
              )
              ? l.awaitingConfirmation
              : l.notConfirmed,
          style: muted,
        ),
    ];
    if (lines.isEmpty) return const SizedBox(height: 4);
    return Padding(
      padding: const EdgeInsets.only(top: 2, bottom: 4),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.end,
        mainAxisSize: MainAxisSize.min,
        children: lines,
      ),
    );
  }
}

/// The chat library's theme, taken from the app's grayscale colour scheme.
fc.ChatTheme chatTheme(ThemeData theme) {
  final c = theme.colorScheme;
  final base = theme.brightness == Brightness.dark
      ? fc.ChatTheme.dark()
      : fc.ChatTheme.light();
  return base.copyWith(
    colors: base.colors.copyWith(
      primary: c.primary,
      onPrimary: c.onPrimary,
      surface: c.surface,
      onSurface: c.onSurface,
      surfaceContainer: c.surfaceContainer,
      surfaceContainerLow: c.surfaceContainerLow,
      surfaceContainerHigh: c.surfaceContainerHigh,
    ),
  );
}
