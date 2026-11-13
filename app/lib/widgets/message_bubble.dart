import 'dart:async';

import 'package:material_ui/material_ui.dart';
import 'package:flutter_chat_core/flutter_chat_core.dart' as fc;
import 'package:intl/intl.dart';

import '../app_scope.dart';
import '../l10n/gen/app_localizations.dart';
import '../screens/chat.dart';
import '../src/rust/api.dart';
import '../src/ui_settings.dart';
import '../theme.dart';
import 'attachments.dart';
import 'grade_badge.dart';
import 'grade_symbol.dart';

/// A text message bubble whose last line carries the channel indication, the time and the
/// delivery tick: `[badges]   11:51 ✓`. The badges start at the bubble's bottom-left corner;
/// the badge area is everything left of the time. The bubble widens to keep them on one line.
class MessageBubble extends StatelessWidget {
  const MessageBubble({
    required this.message,
    required this.isSentByMe,
    required this.item,
    required this.settings,
    this.floor,
    super.key,
  });

  final fc.TextMessage message;
  final bool isSentByMe;

  /// What the core knows about the message (channels, status); `null` before it is known.
  final MessageItem? item;
  final UiSettings settings;

  /// The contact's privacy floor (0 to 5); `null` in group chats, which have none.
  final int? floor;

  @override
  Widget build(BuildContext context) {
    final chat = chatTheme(Theme.of(context));
    final emoji = fc.isOnlyEmoji(message.text);
    final background = emoji
        ? Theme.of(context).colorScheme.surface
        : isSentByMe
        ? chat.colors.primary
        : chat.colors.surfaceContainer;
    final ink = isSentByMe && !emoji
        ? chat.colors.onPrimary
        : chat.colors.onSurface;
    final textStyle = chat.typography.bodyMedium.copyWith(
      color: ink,
      fontSize: emoji ? 48 : null,
    );
    final timeStyle = chat.typography.labelSmall.copyWith(color: ink);
    final status = isSentByMe ? message.resolvedStatus : null;
    final time = message.resolvedTime;

    final footer = Row(
      crossAxisAlignment: CrossAxisAlignment.center,
      children: [
        Expanded(
          child: item == null
              ? const SizedBox.shrink()
              : Indication(
                  key: ValueKey(item!.id),
                  item: item!,
                  floor: floor,
                  background: background,
                  fontSize: timeStyle.fontSize ?? 11,
                  settings: settings,
                ),
        ),
        if (settings.showMessageTimes && time != null) ...[
          const SizedBox(width: 6),
          Text(
            DateFormat.Hm(Localizations.localeOf(context).toString())
                .format(time.toLocal()),
            style: timeStyle,
          ),
        ],
        if (status != null) ...[
          const SizedBox(width: 2),
          Icon(fc.getIconForStatus(status), color: ink, size: 12),
        ],
      ],
    );

    return ClipRRect(
      borderRadius: chat.shape,
      child: Container(
        color: emoji ? null : background,
        padding: const EdgeInsets.fromLTRB(16, 10, 16, 8),
        child: IntrinsicWidth(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              if (item?.attachments.isNotEmpty ?? false)
                AttachmentList(item: item!, background: background),
              if (message.text.isNotEmpty) Text(message.text, style: textStyle),
              const SizedBox(height: 4),
              footer,
            ],
          ),
        ),
      ),
    );
  }
}

/// The channel indication of one message: the full badges, or the grade symbols they collapse
/// into, following the message badges setting. Tapping it shows the full badges for a while.
class Indication extends StatefulWidget {
  const Indication({
    required this.item,
    required this.background,
    required this.fontSize,
    required this.settings,
    this.floor,
    super.key,
  });

  final MessageItem item;
  final Color background;
  final double fontSize;
  final UiSettings settings;
  final int? floor;

  /// How long a tap shows the full badges where the setting has no delay of its own.
  static const tapShows = Duration(seconds: 7);

  /// The clock, in Unix ms; replaced by tests.
  @visibleForTesting
  static int Function() now = () => DateTime.now().millisecondsSinceEpoch;

  @override
  State<Indication> createState() => _IndicationState();
}

class _IndicationState extends State<Indication> {
  /// When each message was first shown, for this session only: nothing is stored.
  static final _firstShown = <String, int>{};

  int? _expandedUntil;
  Timer? _timer;
  int? _timerAt;

  @override
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }

  int get _mode => widget.settings.badgeMode;

  /// When the message arrived here or went out: the latest channel use.
  int get _arrival {
    final uses = widget.item.channels.map((u) => u.atUnixMs.toInt());
    return uses.isEmpty
        ? widget.item.createdUnixMs.toInt()
        : uses.reduce((a, b) => a > b ? a : b);
  }

  /// Whether the full badges show at [now]; also schedules the next change.
  bool _full(int now) {
    if (_mode >= UiSettings.maxBadgeMode) return true;
    final until = _expandedUntil;
    final changes = <int>[];
    var full = false;
    if (until != null && now < until) {
      full = true;
      changes.add(until);
    }
    final delay = UiSettings.badgeDelay(_mode)?.inMilliseconds;
    if (delay != null) {
      final first = _firstShown.putIfAbsent(widget.item.id, () => now);
      // A message already older than the delay when first shown opens collapsed.
      if (first - _arrival < delay) {
        final collapseAt = first + delay;
        if (now < collapseAt) {
          full = true;
          changes.add(collapseAt);
        }
      }
    }
    if (changes.isEmpty) {
      _timer?.cancel();
      _timerAt = null;
    } else {
      _schedule(changes.reduce((a, b) => a > b ? a : b), now);
    }
    return full;
  }

  void _schedule(int at, int now) {
    if (_timerAt == at) return;
    _timer?.cancel();
    _timerAt = at;
    _timer = Timer(Duration(milliseconds: at - now + 20), () {
      _timerAt = null;
      if (mounted) setState(() {});
    });
  }

  void _expand() {
    final shows = UiSettings.badgeDelay(_mode) ?? Indication.tapShows;
    setState(() {
      _expandedUntil = Indication.now() + shows.inMilliseconds;
    });
  }

  @override
  Widget build(BuildContext context) {
    final item = widget.item;
    if (item.channels.isEmpty) return const SizedBox.shrink();
    final l = AppLocalizations.of(context);
    final now = Indication.now();
    final full = _full(now);
    final settings = widget.settings;
    final detailed = settings.badgeDetail == BadgeDetail.detailed;
    final time = DateFormat.Hm(Localizations.localeOf(context).toString());
    final delivered = switch (item.status) {
      MessageStatus.delivered => l.statusDelivered,
      MessageStatus.received => l.statusReceived,
      _ => null,
    };

    // A message sent again over the same channel is recorded once per send; show each channel
    // once, with the first time it carried the message.
    final uses = <(Channel, String), ChannelUseItem>{};
    for (final u in item.channels) {
      final key = (u.channel, u.grade);
      final seen = uses[key];
      if (seen == null || u.atUnixMs < seen.atUnixMs) uses[key] = u;
    }

    final Widget content;
    if (full) {
      // One line, from the bubble's left edge; one badge under another only when even the
      // widest bubble cannot hold them side by side.
      content = OverflowBar(
        spacing: 4,
        overflowSpacing: 2,
        children: [
          for (final u in uses.values)
            GradeBadge(
              label: usedChannelLabel(l, u.channel, u.grade),
              grade: u.grade,
              background: widget.background,
              detail: detailed
                  ? time.format(
                      DateTime.fromMillisecondsSinceEpoch(u.atUnixMs.toInt()),
                    )
                  : null,
            ),
          if (detailed && delivered != null)
            Text(
              delivered,
              style: TextStyle(
                fontSize: 11,
                color: MemColors.onBubble(widget.background),
              ),
            ),
        ],
      );
    } else {
      // One symbol per distinct grade, the least private nearest the time.
      final grades = {for (final u in item.channels) u.grade}.toList()
        ..sort((a, b) => gradeNumber(a).compareTo(gradeNumber(b)));
      final shown = _mode == UiSettings.minBadgeMode
          ? grades.where((g) {
              final n = gradeNumber(g);
              final floor = widget.floor;
              return n >= 4 || (floor != null && n > floor);
            })
          : grades;
      content = Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          for (final g in shown) ...[
            const SizedBox(width: 3),
            GradeSymbol(
              grade: g,
              shape: shapeFor(g, settings.symbolStyle, settings.symbolShape),
              background: widget.background,
              fontSize: widget.fontSize,
            ),
          ],
        ],
      );
    }

    final label = [
      for (final u in uses.values)
        '${usedChannelLabel(l, u.channel, u.grade)} · ${u.grade}',
      ?delivered,
    ].join(', ');
    final reduced = MediaQuery.maybeDisableAnimationsOf(context) ?? false;
    final tappable = _mode < UiSettings.maxBadgeMode;
    return Semantics(
      label: label,
      button: tappable,
      excludeSemantics: true,
      child: GestureDetector(
        behavior: HitTestBehavior.opaque,
        onTap: tappable ? _expand : null,
        // A switch, not a size animation: the bubble measures its own width from this line, and a
        // size animation may not change while it is being measured. The full badges open from
        // the left edge to the right.
        child: AnimatedSwitcher(
          duration: reduced ? Duration.zero : const Duration(milliseconds: 180),
          layoutBuilder: (current, previous) => Stack(
            alignment: AlignmentDirectional.centerStart,
            children: [...previous, ?current],
          ),
          transitionBuilder: (child, animation) => FadeTransition(
            opacity: animation,
            child: SizeTransition(
              sizeFactor: animation,
              axis: Axis.horizontal,
              alignment: AlignmentDirectional.topStart,
              child: child,
            ),
          ),
          child: Align(
            key: ValueKey(full),
            alignment: AlignmentDirectional.centerStart,
            widthFactor: 1,
            child: content,
          ),
        ),
      ),
    );
  }
}
