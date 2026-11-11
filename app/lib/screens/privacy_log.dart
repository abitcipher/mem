import 'package:flutter/material.dart';
import 'package:intl/intl.dart';

import '../app_scope.dart';
import '../l10n/gen/app_localizations.dart';
import '../widgets/grade_badge.dart';

/// Every below-floor send, newest first.
class PrivacyLogScreen extends StatelessWidget {
  const PrivacyLogScreen({super.key});

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final state = AppScope.of(context);
    final log = state.node.privacyLog();
    final rules = state.node.rules();
    final attested = state.node.attestLog();
    final fmt = DateFormat.yMMMd(Localizations.localeOf(context).toString())
        .add_Hm();
    return Scaffold(
      appBar: AppBar(title: Text(l.privacyLog)),
      body: ListView(
        children: [
          if (log.isEmpty)
            Padding(padding: const EdgeInsets.all(24), child: Text(l.logEmpty)),
          for (final e in log)
            ListTile(
              leading: GradeBadge(
                label: channelLabel(l, e.channel),
                grade: e.grade,
              ),
              title: Text(e.contact),
              subtitle: Text(
                '${fmt.format(DateTime.fromMillisecondsSinceEpoch(e.atUnixMs.toInt()))}\n'
                '${l.logEntry(e.grade, e.floor, switch (e.basis) {
                  'once' => l.basisOnce,
                  'new_rule' => l.basisNewRule,
                  _ => l.basisRule,
                })}',
              ),
              isThreeLine: true,
            ),
          if (attested.isNotEmpty) ...[
            const Divider(),
            ListTile(
              title: Text(
                l.attestLog,
                style: const TextStyle(fontWeight: FontWeight.bold),
              ),
            ),
            for (final a in attested)
              ListTile(
                leading: const Icon(Icons.handshake_outlined),
                title: Text(a.label),
                subtitle: Text(
                  '${fmt.format(DateTime.fromMillisecondsSinceEpoch(a.atUnixMs.toInt()))}\n'
                  '${l.attestLogEntry(a.origin, a.sent ? l.attestLogSent : l.attestLogCopied)}',
                ),
                isThreeLine: true,
              ),
          ],
          const Divider(),
          ListTile(
            title: Text(
              l.standingRules,
              style: const TextStyle(fontWeight: FontWeight.bold),
            ),
          ),
          if (rules.isEmpty) ListTile(title: Text(l.noRules)),
          for (final r in rules)
            ListTile(
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
        ],
      ),
    );
  }
}
