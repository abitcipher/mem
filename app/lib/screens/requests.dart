import 'package:flutter/material.dart';
import 'package:intl/intl.dart';

import '../app_scope.dart';
import '../l10n/gen/app_localizations.dart';
import '../src/rust/api.dart';
import '../widgets/grade_badge.dart';
import 'chat.dart';
import 'groups.dart';

/// Contact requests in quarantine: never messages, only a fingerprint, an
/// unverified name and a note, until the user accepts.
class RequestsScreen extends StatelessWidget {
  const RequestsScreen({super.key});

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final state = AppScope.of(context);
    final requests = state.node.requests();
    final invitations = state.node
        .groups()
        .where((g) => g.status == GroupStatusItem.invited)
        .toList();
    return Scaffold(
      appBar: AppBar(title: Text(l.requestsTitle)),
      body: requests.isEmpty && invitations.isEmpty
          ? Center(child: Text(l.requestsEmpty))
          : ListView(
              padding: const EdgeInsets.all(16),
              children: [
                if (invitations.isNotEmpty) ...[
                  Text(
                    l.groupInvitationsTitle,
                    style: Theme.of(context).textTheme.titleMedium,
                  ),
                  Text(l.groupInvitationHelp),
                  for (final g in invitations) _InvitationCard(group: g),
                  const SizedBox(height: 16),
                ],
                if (requests.isNotEmpty) ...[
                  Text(l.requestsHelp),
                  const SizedBox(height: 8),
                  for (final r in requests.reversed) _RequestCard(request: r),
                ],
              ],
            ),
    );
  }
}

class _RequestCard extends StatelessWidget {
  const _RequestCard({required this.request});

  final RequestItem request;

  Future<void> _accept(BuildContext context) async {
    final l = AppLocalizations.of(context);
    final state = AppScope.read(context);
    try {
      final id = await state.node.approveRequest(id: request.id);
      state.changed();
      if (!context.mounted) return;
      showSnack(context, l.requestAccepted);
      await Navigator.of(context).pushReplacement(
        MaterialPageRoute<void>(builder: (_) => ChatScreen(contactId: id)),
      );
    } on Object catch (e) {
      if (context.mounted) showSnack(context, reasonText(context, e));
    }
  }

  Future<void> _decline(BuildContext context, {required bool block}) async {
    final l = AppLocalizations.of(context);
    final state = AppScope.read(context);
    if (block) {
      final sure = await showDialog<bool>(
        context: context,
        builder: (context) => AlertDialog(
          content: Text(l.blockConfirm),
          actions: [
            TextButton(
              onPressed: () => Navigator.pop(context, false),
              child: Text(l.cancel),
            ),
            FilledButton(
              onPressed: () => Navigator.pop(context, true),
              child: Text(l.blockRequest),
            ),
          ],
        ),
      );
      if (sure != true) return;
    }
    try {
      await state.node.declineRequest(id: request.id, block: block);
      state.changed();
    } on Object catch (e) {
      if (context.mounted) showSnack(context, reasonText(context, e));
    }
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final theme = Theme.of(context);
    final when = DateFormat.MMMd(Localizations.localeOf(context).toString())
        .add_Hm();
    final name = request.displayName;
    return Card(
      margin: const EdgeInsets.symmetric(vertical: 8),
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              name == null ? l.remote : l.nameNotVerified(name),
              style: theme.textTheme.titleMedium,
            ),
            const SizedBox(height: 4),
            Text(
              '${l.fingerprint}: ${request.fingerprint}',
              style: const TextStyle(fontFamily: 'monospace', fontSize: 12),
            ),
            const SizedBox(height: 8),
            Text(request.note ?? l.requestNoNote),
            const SizedBox(height: 8),
            Row(
              children: [
                GradeBadge(
                  label: channelLabel(l, request.via.channel),
                  grade: request.via.grade,
                ),
                const SizedBox(width: 8),
                Text(
                  when.format(
                    DateTime.fromMillisecondsSinceEpoch(
                      request.receivedUnixMs.toInt(),
                    ),
                  ),
                  style: theme.textTheme.bodySmall,
                ),
              ],
            ),
            const SizedBox(height: 8),
            Wrap(
              alignment: WrapAlignment.end,
              spacing: 8,
              children: [
                TextButton(
                  onPressed: () => _decline(context, block: true),
                  child: Text(l.blockRequest),
                ),
                OutlinedButton(
                  onPressed: () => _decline(context, block: false),
                  child: Text(l.declineRequest),
                ),
                FilledButton(
                  onPressed: () => _accept(context),
                  child: Text(l.acceptRequest),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}

/// A group invitation from a contact: who invites, who is in it.
class _InvitationCard extends StatelessWidget {
  const _InvitationCard({required this.group});

  final GroupItem group;

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final admin = group.members
        .where((m) => m.role == MemberRoleItem.admin)
        .firstOrNull;
    final others = group.members.where((m) => m.role != MemberRoleItem.admin);
    return Card(
      margin: const EdgeInsets.symmetric(vertical: 8),
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(group.name, style: Theme.of(context).textTheme.titleMedium),
            if (admin != null) Text(l.groupInvitedBy(memberTitle(admin))),
            const SizedBox(height: 4),
            Text(
              others.map(memberTitle).join(', '),
              style: Theme.of(context).textTheme.bodySmall,
            ),
            const SizedBox(height: 8),
            Wrap(
              alignment: WrapAlignment.end,
              spacing: 8,
              children: [
                OutlinedButton(
                  onPressed: () =>
                      answerInvitation(context, group.id, accept: false),
                  child: Text(l.groupDecline),
                ),
                FilledButton(
                  onPressed: () =>
                      answerInvitation(context, group.id, accept: true),
                  child: Text(l.groupJoin),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}
