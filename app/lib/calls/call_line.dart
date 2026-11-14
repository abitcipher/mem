import 'package:intl/intl.dart';
import 'package:material_ui/material_ui.dart';

import '../app_scope.dart';
import '../l10n/gen/app_localizations.dart';
import '../src/rust/api.dart';
import '../widgets/grade_badge.dart';

/// A voice call as a line in the chat: who called, how it went, how long, and the path the
/// voice took (honest provenance, like a message's badge).
class CallLine extends StatelessWidget {
  const CallLine({required this.item, required this.call, super.key});

  final MessageItem item;
  final CallItem call;

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final missed = call.outcome == CallOutcomeItem.missed && !item.outgoing;
    final what = switch (call.outcome) {
      CallOutcomeItem.answered =>
        item.outgoing ? l.callLineOutgoing : l.callLineIncoming,
      CallOutcomeItem.missed => l.callLineMissed,
      CallOutcomeItem.declined => l.callDeclined,
      CallOutcomeItem.busy => l.callBusy,
      CallOutcomeItem.failed => l.callFailed,
      CallOutcomeItem.unanswered => l.callUnanswered,
      CallOutcomeItem.unreachable => l.callUnreachable,
    };
    final s = call.durationS;
    final length = s == 0
        ? ''
        : ' · ${s ~/ 60}:${(s % 60).toString().padLeft(2, '0')}';
    final time = DateFormat.Hm(
      Localizations.localeOf(context).toString(),
    ).format(DateTime.fromMillisecondsSinceEpoch(item.createdUnixMs.toInt()));
    final color = missed ? Theme.of(context).colorScheme.error : null;
    return Center(
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: 6),
        child: Wrap(
          crossAxisAlignment: WrapCrossAlignment.center,
          spacing: 8,
          children: [
            Icon(
              missed
                  ? Icons.call_missed
                  : item.outgoing
                  ? Icons.call_made
                  : Icons.call_received,
              size: 18,
              color: color,
            ),
            Text(
              '$what$length · $time',
              style: Theme.of(context).textTheme.labelMedium
                  ?.copyWith(color: color),
            ),
            for (final u in item.channels)
              GradeBadge(label: channelLabel(l, u.channel), grade: u.grade),
          ],
        ),
      ),
    );
  }
}
