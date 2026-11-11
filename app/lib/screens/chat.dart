import 'package:flutter/material.dart';
import 'package:flutter_chat_core/flutter_chat_core.dart' as fc;
import 'package:flutter_chat_ui/flutter_chat_ui.dart';
import 'package:intl/intl.dart';
// flutter_chat_ui builds on the standalone material_ui package, whose Material type differs
// from package:flutter/material.dart's; its composer needs this ancestor.
import 'package:material_ui/material_ui.dart' as mui;

import '../app_scope.dart';
import '../l10n/gen/app_localizations.dart';
import '../src/rust/api.dart';
import '../src/ui_settings.dart';
import '../widgets/grade_badge.dart';
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
    _controller.setMessages([
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
    ]);
    if (mounted) setState(() {});
  }

  Future<void> _send(String text) async {
    final state = AppScope.read(context);
    try {
      final sent = await state.node.sendText(
        contactId: widget.contactId,
        text: text,
      );
      state.changed();
      // Already on its way over Wi-Fi: no need to ask how it should travel.
      if (mounted && !sent.sendingNearby) {
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

  Widget _chat(BuildContext context, String title) => mui.Material(
    type: mui.MaterialType.transparency,
    child: Chat(
      currentUserId: _me,
      theme: chatTheme(Theme.of(context)),
      chatController: _controller,
      resolveUser: (id) async =>
          fc.User(id: id, name: id == _me ? null : title),
      onMessageSend: _send,
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
                  SimpleTextMessage(
                    message: message,
                    index: index,
                    showTime: settings.showMessageTimes,
                  ),
                  if (item != null)
                    Provenance(
                      item: item,
                      detailed: settings.badgeDetail == BadgeDetail.detailed,
                    ),
                ],
              );
            },
      ),
    ),
  );
}

/// Which channels carried the message, with their grades. Always shown; the detailed form
/// adds when each channel carried it and the delivery status.
class Provenance extends StatelessWidget {
  const Provenance({required this.item, required this.detailed, super.key});

  final MessageItem item;
  final bool detailed;

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final muted = TextStyle(
      fontSize: 11,
      color: Theme.of(context).colorScheme.onSurfaceVariant,
    );
    // Our group message: copies go out by themselves; show how many confirmed.
    final total = item.sentTo;
    final groupLine = total == null
        ? null
        : Text(l.deliveredToCount(item.deliveredTo ?? 0, total), style: muted);
    if (item.channels.isEmpty && groupLine != null) {
      return Padding(
        padding: const EdgeInsets.only(top: 2, bottom: 4),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.end,
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(l.groupSentAutomatically, style: muted),
            groupLine,
          ],
        ),
      );
    }
    if (item.channels.isEmpty) {
      return Padding(
        padding: const EdgeInsets.only(top: 2, bottom: 4),
        child: Text(
          l.tapToSend,
          style: TextStyle(
            fontSize: 11,
            color: Theme.of(context).colorScheme.error,
          ),
        ),
      );
    }
    final time = DateFormat.Hm(Localizations.localeOf(context).toString());
    final badges = Wrap(
      spacing: 4,
      runSpacing: 2,
      children: [
        for (final u in item.channels)
          GradeBadge(
            label: usedChannelLabel(l, u.channel, u.grade),
            grade: u.grade,
            detail: detailed
                ? time.format(
                    DateTime.fromMillisecondsSinceEpoch(u.atUnixMs.toInt()),
                  )
                : null,
          ),
        // Unconfirmed sent messages get their own line below instead.
        if (detailed &&
            (item.status == MessageStatus.delivered ||
                item.status == MessageStatus.received))
          Text(
            item.status == MessageStatus.delivered
                ? l.statusDelivered
                : l.statusReceived,
            style: TextStyle(
              fontSize: 11,
              color: Theme.of(context).colorScheme.onSurfaceVariant,
            ),
          ),
      ],
    );
    if (groupLine != null) {
      return Padding(
        padding: const EdgeInsets.only(top: 2, bottom: 4),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.end,
          mainAxisSize: MainAxisSize.min,
          children: [badges, const SizedBox(height: 2), groupLine],
        ),
      );
    }
    if (!item.outgoing || item.status != MessageStatus.sent) {
      return Padding(
        padding: const EdgeInsets.only(top: 2, bottom: 4),
        child: badges,
      );
    }
    // Sent but not confirmed: Wi-Fi, Bluetooth and the hub carry on by themselves; QR and share
    // need the user.
    final automatic = item.channels.any(
      (u) =>
          u.channel == Channel.lan ||
          u.channel == Channel.ble ||
          u.channel == Channel.wifiDirect ||
          u.channel == Channel.hub,
    );
    return Padding(
      padding: const EdgeInsets.only(top: 2, bottom: 4),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.end,
        mainAxisSize: MainAxisSize.min,
        children: [
          badges,
          const SizedBox(height: 2),
          Text(
            automatic ? l.awaitingConfirmation : l.notConfirmed,
            style: TextStyle(
              fontSize: 11,
              color: Theme.of(context).colorScheme.onSurfaceVariant,
            ),
          ),
        ],
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
