import 'package:material_ui/material_ui.dart';
import 'package:intl/intl.dart';
import 'package:mem_calls/mem_calls.dart';

import '../app_scope.dart';
import '../l10n/gen/app_localizations.dart';
import '../widgets/edit_limit.dart';
import 'settings.dart' show toneName;
import 'shares.dart';
import '../src/rust/api.dart';

class ContactScreen extends StatelessWidget {
  const ContactScreen({required this.contactId, super.key});

  final String contactId;

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final state = AppScope.of(context);
    final c = state.node.contacts().where((c) => c.id == contactId).firstOrNull;
    if (c == null) return Scaffold(appBar: AppBar());
    final title = contactTitle(c);
    final rules = state.node
        .rules()
        .where((r) => r.contactId == null || r.contactId == contactId)
        .toList();
    return Scaffold(
      appBar: AppBar(title: Text(l.contactSettings)),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          Row(
            children: [
              Expanded(
                child: Text(
                  title,
                  style: Theme.of(context).textTheme.headlineSmall,
                ),
              ),
              IconButton(
                icon: const Icon(Icons.edit_outlined),
                tooltip: l.contactAliasEdit,
                onPressed: () => _rename(context, c),
              ),
            ],
          ),
          // With a name of the user's own, the one the contact chose stays visible.
          if (c.alias != null)
            Text(
              c.displayName == null
                  ? l.contactTheirNameNone
                  : l.contactTheirName(c.displayName!),
              style: Theme.of(context).textTheme.bodySmall,
            ),
          const SizedBox(height: 4),
          Row(
            children: [
              Icon(
                c.metInPerson ? Icons.handshake_outlined : Icons.public,
                size: 18,
              ),
              const SizedBox(width: 6),
              Text(c.metInPerson ? l.metInPerson : l.remote),
            ],
          ),
          const SizedBox(height: 16),
          Text(
            l.fingerprint,
            style: const TextStyle(fontWeight: FontWeight.bold),
          ),
          SelectableText(
            c.fingerprint,
            style: const TextStyle(fontFamily: 'monospace'),
          ),
          const SizedBox(height: 8),
          Text(c.sas, style: const TextStyle(fontSize: 26)),
          const Divider(height: 32),
          Text(
            l.privacyFloor,
            style: const TextStyle(fontWeight: FontWeight.bold),
          ),
          const SizedBox(height: 4),
          Text(l.privacyFloorHelp),
          const SizedBox(height: 8),
          DropdownButton<int>(
            isExpanded: true,
            value: c.floor,
            items: [
              for (var g = 0; g <= 4; g++)
                DropdownMenuItem(value: g, child: Text(gradeLabel(l, g))),
            ],
            onChanged: (g) async {
              if (g == null) return;
              await state.node.setFloor(contactId: contactId, floor: g);
              state.changed();
            },
          ),
          const Divider(height: 32),
          _ContactEmail(contactId: contactId, email: c.email),
          const Divider(height: 32),
          Text(
            l.editLimitContact,
            style: const TextStyle(fontWeight: FontWeight.bold),
          ),
          DropdownButton<int>(
            isExpanded: true,
            // -1 stands for "as the global setting".
            value: c.editLimit ?? -1,
            items: [
              DropdownMenuItem(
                value: -1,
                child: Text(
                  l.editLimitAsGlobal(
                    editLimitLabel(l, state.node.editLimit()),
                  ),
                ),
              ),
              for (var p = 0; p < editLimitPositions; p++)
                DropdownMenuItem(value: p, child: Text(editLimitLabel(l, p))),
            ],
            onChanged: (v) async {
              if (v == null) return;
              await state.node.setEditLimit(
                contactId: contactId,
                position: v < 0 ? null : v,
              );
              state.changed();
            },
          ),
          const Divider(height: 32),
          Text(
            l.sharesTitle,
            style: const TextStyle(fontWeight: FontWeight.bold),
          ),
          ListTile(
            contentPadding: EdgeInsets.zero,
            leading: const Icon(Icons.folder_shared_outlined),
            title: Text(l.sharesMine(title)),
            onTap: () => Navigator.of(context).push(
              MaterialPageRoute<void>(
                builder: (_) => SharesScreen(contactId: contactId),
              ),
            ),
          ),
          ListTile(
            contentPadding: EdgeInsets.zero,
            leading: const Icon(Icons.folder_outlined),
            title: Text(l.sharedWithMe),
            onTap: () => Navigator.of(context).push(
              MaterialPageRoute<void>(
                builder: (_) => SharedWithMeScreen(contactId: contactId),
              ),
            ),
          ),
          const Divider(height: 32),
          SwitchListTile(
            contentPadding: EdgeInsets.zero,
            secondary: const Icon(Icons.tablet_android_outlined),
            title: Text(l.deviceHubUse),
            subtitle: Text(
              [
                l.deviceHubUseHelp(title),
                if (c.hubOnline) l.deviceHubConnected,
              ].join('\n\n'),
            ),
            value: c.useAsHub,
            onChanged: (on) async {
              try {
                await state.node.setUseDeviceHub(
                  contactId: contactId,
                  enabled: on,
                );
                state.changed();
              } on Object catch (e) {
                if (context.mounted) showSnack(context, reasonText(context, e));
              }
            },
          ),
          if (state.calls.enabled) ...[
            const Divider(height: 32),
            _ContactRingtone(contactId: contactId, ringtone: c.ringtone),
          ],
          const Divider(height: 32),
          Text(
            l.standingRules,
            style: const TextStyle(fontWeight: FontWeight.bold),
          ),
          if (rules.isEmpty) Text(l.noRules),
          for (final r in rules)
            ListTile(
              contentPadding: EdgeInsets.zero,
              title: Text('${channelLabel(l, r.channel)} · ≤ ${r.maxGrade}'),
              subtitle: Text(
                [
                  r.contact ?? l.allContacts,
                  if (r.untilUnixMs case final until?)
                    l.ruleUntil(
                      DateFormat.yMMMd(
                        Localizations.localeOf(context).toString(),
                      ).add_Hm().format(
                        DateTime.fromMillisecondsSinceEpoch(until.toInt()),
                      ),
                    ),
                ].join(' · '),
              ),
              trailing: TextButton(
                child: Text(l.revoke),
                onPressed: () async {
                  await state.node.revokeRule(id: r.id);
                  state.changed();
                },
              ),
            ),
          const Divider(height: 32),
          Align(
            alignment: Alignment.centerLeft,
            child: TextButton.icon(
              style: TextButton.styleFrom(
                foregroundColor: Theme.of(context).colorScheme.error,
              ),
              icon: const Icon(Icons.person_remove_outlined),
              label: Text(l.removeContact),
              onPressed: () => _remove(context),
            ),
          ),
        ],
      ),
    );
  }

  /// The name the user gives this contact, kept on this phone only.
  Future<void> _rename(BuildContext context, ContactItem c) async {
    final l = AppLocalizations.of(context);
    final state = AppScope.read(context);
    final controller = TextEditingController(text: c.alias ?? '');
    final name = await showDialog<String>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(l.contactAliasTitle),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            TextField(
              controller: controller,
              autofocus: true,
              maxLength: 64,
              decoration: InputDecoration(hintText: c.displayName),
              onSubmitted: (v) => Navigator.pop(context, v),
            ),
            Text(l.contactAliasHelp),
          ],
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
    if (name == null || !context.mounted) return;
    final trimmed = name.trim();
    try {
      await state.node.setContactAlias(
        contactId: contactId,
        alias: trimmed.isEmpty ? null : trimmed,
      );
      state.changed();
    } on Object catch (e) {
      if (context.mounted) showSnack(context, reasonText(context, e));
    }
  }

  /// Asks, then removes the contact (it cannot be undone: pairing again is needed).
  Future<void> _remove(BuildContext context) async {
    final l = AppLocalizations.of(context);
    final state = AppScope.read(context);
    final ok = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(l.removeContactTitle),
        content: Text(l.removeContactBody),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context, false),
            child: Text(l.cancel),
          ),
          FilledButton(
            style: FilledButton.styleFrom(
              backgroundColor: Theme.of(context).colorScheme.error,
            ),
            onPressed: () => Navigator.pop(context, true),
            child: Text(l.removeContact),
          ),
        ],
      ),
    );
    if (ok != true || !context.mounted) return;
    await state.node.removeContact(contactId: contactId);
    state.changed();
    if (context.mounted) {
      Navigator.of(context).popUntil((route) => route.isFirst);
    }
  }
}

/// The contact's email address: typed here, kept on this phone.
class _ContactEmail extends StatefulWidget {
  const _ContactEmail({required this.contactId, required this.email});

  final String contactId;
  final String? email;

  @override
  State<_ContactEmail> createState() => _ContactEmailState();
}

class _ContactEmailState extends State<_ContactEmail> {
  late final _controller = TextEditingController(text: widget.email ?? '');

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final state = AppScope.read(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            Expanded(
              child: TextField(
                controller: _controller,
                keyboardType: TextInputType.emailAddress,
                autocorrect: false,
                decoration: InputDecoration(labelText: l.contactEmail),
              ),
            ),
            TextButton(
              onPressed: () async {
                final text = _controller.text.trim();
                try {
                  await state.node.setContactEmail(
                    contactId: widget.contactId,
                    address: text.isEmpty ? null : text,
                  );
                  state.changed();
                } on Object catch (e) {
                  if (context.mounted) {
                    showSnack(context, reasonText(context, e));
                  }
                }
              },
              child: Text(l.contactEmailSave),
            ),
          ],
        ),
        Text(l.contactEmailHelp, style: Theme.of(context).textTheme.bodySmall),
      ],
    );
  }
}

/// The incoming-call tone for this contact: the default, one of the tones, or off. Kept in the
/// encrypted state (which tone a contact has would show who the contacts are).
class _ContactRingtone extends StatelessWidget {
  const _ContactRingtone({required this.contactId, required this.ringtone});

  final String contactId;
  final String? ringtone;

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final state = AppScope.read(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          l.contactRingtone,
          style: const TextStyle(fontWeight: FontWeight.bold),
        ),
        DropdownButton<String>(
          isExpanded: true,
          value: ringtone ?? '',
          items: [
            DropdownMenuItem(value: '', child: Text(l.toneDefault)),
            for (final t in MemTone.forIncoming)
              DropdownMenuItem(value: t.id, child: Text(toneName(l, t))),
            DropdownMenuItem(value: 'off', child: Text(l.toneOff)),
          ],
          onChanged: (v) async {
            if (v == null) return;
            await state.node.setContactRingtone(
              contactId: contactId,
              ringtone: v.isEmpty ? null : v,
            );
            state.changed();
          },
        ),
      ],
    );
  }
}
