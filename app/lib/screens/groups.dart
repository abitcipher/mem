import 'package:flutter/material.dart';
import 'package:flutter_chat_core/flutter_chat_core.dart' as fc;
import 'package:flutter_chat_ui/flutter_chat_ui.dart';
// flutter_chat_ui builds on the standalone material_ui package; its composer needs this ancestor.
import 'package:material_ui/material_ui.dart' as mui;

import '../app_scope.dart';
import '../l10n/gen/app_localizations.dart';
import '../src/rust/api.dart';
import '../src/ui_settings.dart';
import 'chat.dart';

/// A member's name as shown: the name from their card, else the start of their fingerprint.
String memberTitle(GroupMemberItem m) =>
    m.displayName ?? m.fingerprint.split(' ').take(2).join(' ');

/// Pick contacts and a name; we become the admin.
class CreateGroupScreen extends StatefulWidget {
  const CreateGroupScreen({super.key});

  @override
  State<CreateGroupScreen> createState() => _CreateGroupScreenState();
}

class _CreateGroupScreenState extends State<CreateGroupScreen> {
  static const _maxInvitees = 15;
  final _name = TextEditingController();
  final _picked = <String>{};
  bool _busy = false;

  @override
  void dispose() {
    _name.dispose();
    super.dispose();
  }

  Future<void> _create() async {
    final state = AppScope.read(context);
    setState(() => _busy = true);
    try {
      final id = await state.node.createGroup(
        name: _name.text,
        memberIds: _picked.toList(),
      );
      state.changed();
      if (!mounted) return;
      await Navigator.of(context).pushReplacement(
        MaterialPageRoute<void>(builder: (_) => GroupChatScreen(groupId: id)),
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
    final contacts = AppScope.of(context).node
        .contacts()
        .where((c) => !c.requested)
        .toList();
    final ready = !_busy && _name.text.trim().isNotEmpty && _picked.isNotEmpty;
    return Scaffold(
      appBar: AppBar(title: Text(l.newGroup)),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          TextField(
            controller: _name,
            maxLength: 64,
            decoration: InputDecoration(labelText: l.groupName),
            onChanged: (_) => setState(() {}),
          ),
          Text(l.groupCreateHelp),
          const SizedBox(height: 16),
          Text(
            l.groupMembersPick,
            style: Theme.of(context).textTheme.titleSmall,
          ),
          for (final c in contacts)
            CheckboxListTile(
              contentPadding: EdgeInsets.zero,
              value: _picked.contains(c.id),
              title: Text(contactTitle(c)),
              onChanged:
                  !_picked.contains(c.id) && _picked.length >= _maxInvitees
                  ? null
                  : (on) => setState(
                      () =>
                          on == true ? _picked.add(c.id) : _picked.remove(c.id),
                    ),
            ),
          const SizedBox(height: 16),
          FilledButton(
            onPressed: ready ? _create : null,
            child: Text(l.groupCreate),
          ),
        ],
      ),
    );
  }
}

/// A group's conversation. Our message goes to every member as its own copy, automatically.
class GroupChatScreen extends StatefulWidget {
  const GroupChatScreen({required this.groupId, super.key});

  final String groupId;

  @override
  State<GroupChatScreen> createState() => _GroupChatScreenState();
}

class _GroupChatScreenState extends State<GroupChatScreen> {
  static const _me = 'me';
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

  void _sync() {
    final items = _state!.node.groupMessages(groupId: widget.groupId);
    _items = {for (final m in items) m.id: m};
    _controller.setMessages([
      for (final m in items)
        fc.Message.text(
          id: m.id,
          authorId: m.outgoing ? _me : m.senderId,
          createdAt: DateTime.fromMillisecondsSinceEpoch(
            m.createdUnixMs.toInt(),
          ),
          text: m.text,
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
      await state.node.sendGroupText(groupId: widget.groupId, text: text);
      state.changed();
    } on Object catch (e) {
      if (mounted) showSnack(context, reasonText(context, e));
    }
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final state = AppScope.of(context);
    final group = state.node
        .groups()
        .where((g) => g.id == widget.groupId)
        .firstOrNull;
    if (group == null) {
      return Scaffold(appBar: AppBar());
    }
    final names = {for (final m in group.members) m.contactId: memberTitle(m)};
    final joined = group.members
        .where((m) => m.role != MemberRoleItem.invited)
        .length;
    final banner = switch (group.status) {
      GroupStatusItem.invited => l.groupInvitedBanner,
      GroupStatusItem.joining => l.groupJoiningBanner,
      GroupStatusItem.left => l.groupLeftBanner,
      GroupStatusItem.removed => l.groupRemovedBanner,
      GroupStatusItem.member => null,
    };
    return Scaffold(
      appBar: AppBar(
        title: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(group.name),
            Text(
              l.groupMembersCount(joined),
              style: Theme.of(context).textTheme.labelSmall,
            ),
          ],
        ),
        actions: [
          IconButton(
            tooltip: l.groupInfo,
            icon: const Icon(Icons.info_outline),
            onPressed: () => Navigator.of(context).push(
              MaterialPageRoute<void>(
                builder: (_) => GroupInfoScreen(groupId: widget.groupId),
              ),
            ),
          ),
        ],
      ),
      body: Column(
        children: [
          if (banner != null)
            MaterialBanner(
              leading: const Icon(Icons.group_outlined),
              content: Text(banner),
              actions: group.status == GroupStatusItem.invited
                  ? [
                      TextButton(
                        onPressed: () => answerInvitation(
                          context,
                          group.id,
                          accept: false,
                          leaveScreen: true,
                        ),
                        child: Text(l.groupDecline),
                      ),
                      FilledButton(
                        onPressed: () => answerInvitation(
                          context,
                          group.id,
                          accept: true,
                          leaveScreen: true,
                        ),
                        child: Text(l.groupJoin),
                      ),
                    ]
                  : const [SizedBox.shrink()],
            ),
          Expanded(
            child: mui.Material(
              type: mui.MaterialType.transparency,
              child: Chat(
                currentUserId: _me,
                theme: chatTheme(Theme.of(context)),
                chatController: _controller,
                resolveUser: (id) async => fc.User(
                  id: id,
                  name: id == _me ? null : names[id] ?? id.substring(0, 8),
                ),
                onMessageSend: group.status == GroupStatusItem.member
                    ? _send
                    : null,
                builders: fc.Builders(
                  composerBuilder: group.status == GroupStatusItem.member
                      ? (context) => Composer(
                          hintText: AppLocalizations.of(context).typeMessage,
                        )
                      : (_) => const SizedBox.shrink(),
                  textMessageBuilder:
                      (
                        context,
                        message,
                        index, {
                        required isSentByMe,
                        groupStatus,
                      }) {
                        final item = _items[message.id];
                        final settings = AppScope.read(context).settings;
                        return Column(
                          crossAxisAlignment: isSentByMe
                              ? CrossAxisAlignment.end
                              : CrossAxisAlignment.start,
                          children: [
                            if (!isSentByMe)
                              Padding(
                                padding: const EdgeInsets.only(bottom: 2),
                                child: Text(
                                  names[message.authorId] ??
                                      message.authorId.substring(0, 8),
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
                                detailed:
                                    settings.badgeDetail ==
                                    BadgeDetail.detailed,
                              ),
                          ],
                        );
                      },
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }
}

/// Joins or declines an invitation (from the chat banner or the Requests screen).
Future<void> answerInvitation(
  BuildContext context,
  String groupId, {
  required bool accept,
  bool leaveScreen = false,
}) async {
  final state = AppScope.read(context);
  try {
    await state.node.answerGroupInvitation(groupId: groupId, accept: accept);
    state.changed();
    if (!accept && leaveScreen && context.mounted) {
      Navigator.of(context).pop();
    }
  } on Object catch (e) {
    if (context.mounted) showSnack(context, reasonText(context, e));
  }
}

/// Members, roles and the admin's controls.
class GroupInfoScreen extends StatelessWidget {
  const GroupInfoScreen({required this.groupId, super.key});

  final String groupId;

  Future<bool> _confirm(
    BuildContext context,
    String text,
    String action,
  ) async {
    final l = AppLocalizations.of(context);
    return await showDialog<bool>(
          context: context,
          builder: (context) => AlertDialog(
            content: Text(text),
            actions: [
              TextButton(
                onPressed: () => Navigator.pop(context, false),
                child: Text(l.cancel),
              ),
              FilledButton(
                onPressed: () => Navigator.pop(context, true),
                child: Text(action),
              ),
            ],
          ),
        ) ??
        false;
  }

  Future<void> _run(BuildContext context, Future<void> Function() f) async {
    final state = AppScope.read(context);
    try {
      await f();
      state.changed();
    } on Object catch (e) {
      if (context.mounted) showSnack(context, reasonText(context, e));
    }
  }

  Future<void> _invite(BuildContext context, GroupItem g) async {
    final l = AppLocalizations.of(context);
    final state = AppScope.read(context);
    final inGroup = g.members.map((m) => m.contactId).toSet();
    final candidates = state.node
        .contacts()
        .where((c) => !c.requested && !inGroup.contains(c.id))
        .toList();
    final picked = await showModalBottomSheet<String>(
      context: context,
      showDragHandle: true,
      builder: (context) => SafeArea(
        child: ListView(
          shrinkWrap: true,
          children: [
            ListTile(title: Text(l.groupInvite)),
            for (final c in candidates)
              ListTile(
                leading: const Icon(Icons.person_outline),
                title: Text(contactTitle(c)),
                onTap: () => Navigator.pop(context, c.id),
              ),
          ],
        ),
      ),
    );
    if (picked == null || !context.mounted) return;
    await _run(
      context,
      () => state.node.inviteToGroup(groupId: g.id, contactId: picked),
    );
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final state = AppScope.of(context);
    final g = state.node.groups().where((g) => g.id == groupId).firstOrNull;
    if (g == null) return Scaffold(appBar: AppBar());
    final live = g.status == GroupStatusItem.member;
    return Scaffold(
      appBar: AppBar(title: Text(g.name)),
      body: ListView(
        children: [
          for (final m in g.members)
            ListTile(
              leading: CircleAvatar(
                child: Text(memberTitle(m).characters.first.toUpperCase()),
              ),
              title: Text(memberTitle(m)),
              subtitle: Text(
                [
                  if (m.isMe) l.groupYou,
                  if (m.role == MemberRoleItem.admin) l.groupAdmin,
                  if (m.role == MemberRoleItem.invited) l.groupInvitedRole,
                  if (!m.isMe && !m.isContact) l.groupNotAContact,
                  m.fingerprint,
                ].join(' · '),
              ),
              trailing: PopupMenuButton<String>(
                itemBuilder: (_) => [
                  if (!m.isMe &&
                      !m.isContact &&
                      m.role != MemberRoleItem.invited)
                    PopupMenuItem(
                      value: 'add',
                      child: Text(l.groupAddAsContact),
                    ),
                  if (g.isAdmin && live && !m.isMe)
                    PopupMenuItem(value: 'remove', child: Text(l.groupRemove)),
                ],
                onSelected: (v) async {
                  if (v == 'add') {
                    final ok = await _confirm(
                      context,
                      l.groupAddAsContactHelp(memberTitle(m)),
                      l.groupAddAsContact,
                    );
                    if (ok && context.mounted) {
                      await _run(
                        context,
                        () => state.node.addMemberAsContact(
                          contactId: m.contactId,
                        ),
                      );
                    }
                  } else if (v == 'remove') {
                    await _run(
                      context,
                      () => state.node.removeFromGroup(
                        groupId: g.id,
                        memberId: m.contactId,
                      ),
                    );
                  }
                },
              ),
            ),
          const Divider(),
          if (g.isAdmin && live)
            ListTile(
              leading: const Icon(Icons.person_add_alt),
              title: Text(l.groupInvite),
              onTap: () => _invite(context, g),
            ),
          if (live || g.status == GroupStatusItem.joining)
            ListTile(
              leading: Icon(
                Icons.logout,
                color: Theme.of(context).colorScheme.error,
              ),
              title: Text(g.isAdmin ? l.groupEnd : l.groupLeave),
              onTap: () async {
                final ok = await _confirm(
                  context,
                  g.isAdmin ? l.groupEndConfirm : l.groupLeaveConfirm,
                  g.isAdmin ? l.groupEnd : l.groupLeave,
                );
                if (ok && context.mounted) {
                  await _run(
                    context,
                    () => state.node.leaveGroup(groupId: g.id),
                  );
                }
              },
            ),
        ],
      ),
    );
  }
}
