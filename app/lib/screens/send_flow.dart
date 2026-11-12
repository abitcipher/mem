import 'dart:io';
import 'dart:math' as math;

import 'package:material_ui/material_ui.dart';
import 'package:flutter/scheduler.dart';
import 'package:flutter/services.dart';
import 'package:share_plus/share_plus.dart';

import '../app_scope.dart';
import '../src/ui_settings.dart';
import '../widgets/help.dart';
import '../l10n/gen/app_localizations.dart';
import '../src/platform.dart';
import '../src/rust/api.dart';
import '../widgets/grade_badge.dart';
import '../widgets/qr.dart';
import '../widgets/glow.dart';
import '../theme.dart';

/// Lets the user choose how a queued message travels, asks for consent below the floor,
/// and hands the bundle to the chosen channel.
Future<void> startSendFlow(BuildContext context, String messageId) async {
  final state = AppScope.read(context);
  final l = AppLocalizations.of(context);
  final locale = localeCode(context);
  final options = state.node.channelOptions(
    messageId: messageId,
    locale: locale,
  );
  if (options.isEmpty) {
    showSnack(context, l.reason_unknown_message);
    return;
  }
  final lanWaiting = options.any(
    (o) =>
        (o.channel == Channel.lan ||
            o.channel == Channel.ble ||
            o.channel == Channel.wifiDirect) &&
        !o.reachable,
  );
  final usable = options.where((o) => o.fits && o.reachable && !o.needsConsent);
  final recommended = usable.isEmpty ? null : usable.first;
  // As tall as its channels, scrolling when they do not fit (small phones, more channels).
  final channel = await showModalBottomSheet<Channel>(
    context: context,
    showDragHandle: true,
    isScrollControlled: true,
    builder: (context) => SafeArea(
      child: SingleChildScrollView(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 0, 16, 8),
              child: Text(
                l.chooseChannel,
                style: Theme.of(context).textTheme.titleMedium,
              ),
            ),
            for (final o in options)
              RainbowGlow(
                // Highlight the recommended choice: the first usable channel that needs no consent.
                active: o == recommended,
                child: ListTile(
                  enabled: o.fits && o.reachable,
                  leading: Icon(channelIcon(o.channel)),
                  title: Row(
                    children: [
                      Flexible(child: Text(o.name)),
                      const SizedBox(width: 8),
                      GradeBadge(
                        label: channelLabel(l, o.channel),
                        grade: o.grade,
                      ),
                    ],
                  ),
                  subtitle: !o.fits
                      ? Text(l.tooLarge)
                      : !o.reachable
                      ? Text(l.notNearby)
                      : o.needsConsent
                      ? Text(l.needsConsent)
                      : o.channel == Channel.courier
                      ? Text(l.courierHint)
                      : null,
                  onTap: () => Navigator.pop(context, o.channel),
                ),
              ),
            if (lanWaiting)
              ListTile(
                leading: const Icon(Icons.schedule),
                title: Text(l.waitForWifi),
                subtitle: Text(l.waitForWifiHint),
                onTap: () => Navigator.pop(context),
              ),
          ],
        ),
      ),
    ),
  );
  if (channel == null || !context.mounted) return;

  try {
    var outcome = await state.node.transmit(
      messageId: messageId,
      channel: channel,
      locale: locale,
    );
    while (true) {
      if (!context.mounted) return;
      switch (outcome) {
        case TransmitOutcome_Ready():
          await _deliver(context, outcome);
          return;
        case TransmitOutcome_Sending():
          showSnack(context, l.sendingNearby);
          return;
        case TransmitOutcome_CourierQueued():
          showSnack(context, l.courierQueued);
          return;
        case TransmitOutcome_PlainReady(:final op, :final text):
          // The second confirmation: the exact text, then the share sheet.
          final done = await Navigator.of(context).push<bool>(
            MaterialPageRoute(
              fullscreenDialog: true,
              builder: (_) => _PlainSendScreen(text: text),
            ),
          );
          await state.node.transmitDone(op: op, done: done ?? false);
          state.changed();
          return;
        case TransmitOutcome_ConsentRequired(:final field0):
          final answer = await showConsentDialog(context, field0);
          if (answer == null || !context.mounted) return;
          final next = await state.node.giveConsent(
            request: ConsentRequestRef(
              offerId: field0.offerId,
              messageId: field0.messageId,
              channel: field0.channel,
              grade: field0.grade,
            ),
            choice: answer.choice,
            ruleDays: answer.days,
            locale: locale,
          );
          state.changed();
          outcome = next;
        case TransmitOutcome_Rejected(:final reason):
          showSnack(context, reasonText(context, reason));
          return;
      }
    }
  } on Object catch (e) {
    if (context.mounted) showSnack(context, reasonText(context, e));
  }
}

Future<void> _deliver(BuildContext context, TransmitOutcome_Ready ready) async {
  final state = AppScope.read(context);
  final done = await Navigator.of(context).push<bool>(
    MaterialPageRoute(
      fullscreenDialog: true,
      builder: (_) => ready.channel == Channel.qr && ready.animation != null
          ? AnimatedQrScreen(animation: ready.animation!)
          : _ShareSendScreen(text: ready.text),
    ),
  );
  await state.node.transmitDone(op: ready.op, done: done ?? false);
  state.changed();
}

/// The consent dialog. Text comes from the core's disclosure generator; this widget only
/// styles it. A rule for the contact lasts `days` (1, 7, 30) or, when `null`,
/// until revoked.
Future<({ConsentChoice choice, int? days})?> showConsentDialog(
  BuildContext context,
  ConsentRequest req,
) async {
  final l = AppLocalizations.of(context);
  Widget section(
    String title,
    List<String> items,
    IconData icon,
    Color color,
  ) => items.isEmpty
      ? const SizedBox.shrink()
      : Padding(
          padding: const EdgeInsets.only(top: 12),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(title, style: const TextStyle(fontWeight: FontWeight.bold)),
              for (final item in items)
                Padding(
                  padding: const EdgeInsets.only(top: 4),
                  child: Row(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Icon(icon, size: 18, color: color),
                      const SizedBox(width: 6),
                      Expanded(child: Text(item)),
                    ],
                  ),
                ),
            ],
          ),
        );

  final choice = await showDialog<ConsentChoice>(
    context: context,
    builder: (context) => AlertDialog(
      icon: Icon(Icons.privacy_tip_outlined, color: MemColors.exposed(context)),
      title: Text(l.consentTitle),
      content: SingleChildScrollView(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(req.title),
            const SizedBox(height: 8),
            Text(
              l.whoCanSee(req.observers),
              style: const TextStyle(fontStyle: FontStyle.italic),
            ),
            section(
              l.theyLearn,
              req.exposed,
              Icons.visibility,
              MemColors.exposed(context),
            ),
            section(
              l.theyLearnPartly,
              req.limited,
              Icons.visibility_outlined,
              MemColors.limited(context),
            ),
            section(
              l.staysPrivate,
              req.protected,
              Icons.lock,
              MemColors.protected(context),
            ),
          ],
        ),
      ),
      actionsOverflowDirection: VerticalDirection.up,
      actionsOverflowButtonSpacing: 4,
      actions: [
        TextButton(
          onPressed: () => Navigator.pop(context),
          child: Text(l.dontSend),
        ),
        // P5 is always this message only.
        if (req.grade < 5)
          TextButton(
            onPressed: () => Navigator.pop(context, ConsentChoice.thisContact),
            child: Text(l.allowContact),
          ),
        FilledButton(
          onPressed: () => Navigator.pop(context, ConsentChoice.once),
          child: Text(l.allowOnce),
        ),
      ],
    ),
  );
  if (choice == null) return null;
  if (choice != ConsentChoice.thisContact) return (choice: choice, days: null);
  if (!context.mounted) return null;
  // A standing rule is a bigger decision: confirm it explicitly, with how long it lasts.
  final days = await showDialog<int>(
    context: context,
    builder: (context) => const _RuleDurationDialog(),
  );
  if (days == null) return null;
  return (choice: ConsentChoice.thisContact, days: days == 0 ? null : days);
}

/// Confirms a standing rule and asks how long it lasts. Pops the days (1, 7, 30), 0 for
/// "until I revoke", or `null` on Cancel.
class _RuleDurationDialog extends StatefulWidget {
  const _RuleDurationDialog();

  @override
  State<_RuleDurationDialog> createState() => _RuleDurationDialogState();
}

class _RuleDurationDialogState extends State<_RuleDurationDialog> {
  int _days = 7;

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final options = {
      1: l.ruleDays1,
      7: l.ruleDays7,
      30: l.ruleDays30,
      0: l.ruleUntilRevoked,
    };
    return AlertDialog(
      title: Text(l.confirmAlwaysTitle),
      content: SingleChildScrollView(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(l.confirmAlwaysBody),
            const SizedBox(height: 12),
            Text(
              l.ruleHowLong,
              style: const TextStyle(fontWeight: FontWeight.bold),
            ),
            RadioGroup<int>(
              groupValue: _days,
              onChanged: (v) => setState(() => _days = v ?? _days),
              child: Column(
                children: [
                  for (final e in options.entries)
                    RadioListTile<int>(
                      contentPadding: EdgeInsets.zero,
                      dense: true,
                      value: e.key,
                      title: Text(e.value),
                    ),
                ],
              ),
            ),
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.pop(context),
          child: Text(l.cancel),
        ),
        FilledButton(
          onPressed: () => Navigator.pop(context, _days),
          child: Text(l.allow),
        ),
      ],
    );
  }
}

/// Shows a fountain-coded transfer as an endless loop of QR frames. The receiver needs any `framesNeeded` of them, in any order.
///
/// The code is as large as the screen allows, the screen stays bright and awake, and the user
/// picks the pace (density and frame rate); the choice is remembered.
class AnimatedQrScreen extends StatefulWidget {
  const AnimatedQrScreen({
    required this.animation,
    this.title,
    this.hint,
    this.pace,
    super.key,
  });

  final QrAnimation animation;

  /// The pace to start at instead of the remembered one (which a choice here still updates).
  final QrPace? pace;

  /// Overrides for other transfers (a backup); default: a message.
  final String? title;
  final String? hint;

  @override
  State<AnimatedQrScreen> createState() => _AnimatedQrScreenState();
}

class _AnimatedQrScreenState extends State<AnimatedQrScreen>
    with SingleTickerProviderStateMixin {
  late final Ticker _ticker = createTicker(_onTick);
  int _n = 0;
  QrView? _frame;
  late double _brightness = AppScope.read(context).settings.codeBrightness;

  QrAnimation get _animation => widget.animation;

  @override
  void initState() {
    super.initState();
    _animation.setPace(
      pace: widget.pace ?? AppScope.read(context).settings.qrPace,
    );
    _restart();
  }

  /// Starts the loop from frame 0 at the current pace. A single-frame transfer needs no loop.
  void _restart() {
    _ticker.stop();
    _n = 0;
    _frame = _animation.frame(n: 0);
    if (_animation.framesNeeded() > 1) _ticker.start();
  }

  /// Frames change on display refreshes, at the pace's rate, so the camera never sees a frame
  /// drawn halfway. A late tick skips frame numbers; the fountain code does not mind.
  void _onTick(Duration elapsed) {
    final n =
        elapsed.inMicroseconds *
        _animation.framesPerSecond() ~/
        Duration.microsecondsPerSecond;
    if (n == _n) return;
    setState(() {
      _n = n;
      _frame = _animation.frame(n: n);
    });
  }

  void _setPace(QrPace pace) {
    AppScope.read(context).updateSettings((s) => s.qrPace = pace);
    setState(() {
      _animation.setPace(pace: pace);
      _restart();
    });
  }

  void _setBrightness(double value, {bool remember = false}) {
    setState(() => _brightness = value);
    PlatformServices.setCodeBrightness(value);
    if (remember) {
      AppScope.read(context).updateSettings((s) => s.codeBrightness = value);
    }
  }

  @override
  void dispose() {
    _ticker.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final frame = _frame;
    final controls = Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        SegmentedButton<QrPace>(
          showSelectedIcon: false,
          segments: [
            ButtonSegment(value: QrPace.easy, label: Text(l.qrPaceEasy)),
            ButtonSegment(value: QrPace.normal, label: Text(l.qrPaceNormal)),
            ButtonSegment(value: QrPace.fast, label: Text(l.qrPaceFast)),
          ],
          selected: {_animation.pace()},
          onSelectionChanged: (s) => _setPace(s.first),
        ),
        // A bright screen washes out in some cameras: the user dims it; remembered.
        Row(
          children: [
            Tooltip(
              message: l.codeBrightness,
              child: const Icon(Icons.brightness_6, size: 20),
            ),
            Expanded(
              child: Slider(
                min: UiSettings.minCodeBrightness,
                value: _brightness,
                semanticFormatterCallback: (v) =>
                    '${l.codeBrightness} ${(v * 100).round()} %',
                onChanged: _setBrightness,
                onChangeEnd: (v) => _setBrightness(v, remember: true),
              ),
            ),
          ],
        ),
        FilledButton(
          onPressed: () => Navigator.pop(context, true),
          child: Text(l.done),
        ),
      ],
    );
    return ShowingCode(
      child: Scaffold(
        appBar: AppBar(
          title: Text(widget.title ?? l.showQrTitle),
          actions: [
            HelpAction(
              title: widget.title ?? l.showQrTitle,
              text:
                  '${widget.hint ?? l.showQrHint}\n\n'
                  '${l.animatedQrHint(_animation.framesNeeded())}\n\n'
                  '${l.qrPaceHint}',
            ),
          ],
        ),
        body: SafeArea(
          child: LayoutBuilder(
            builder: (context, box) {
              final landscape = box.maxWidth > box.maxHeight;
              // The code takes what the controls leave: the full width in portrait (less a
              // margin for the outline), the full height in landscape.
              final side = landscape
                  ? math.min(box.maxHeight - 16, box.maxWidth * 0.62)
                  : math.min(box.maxWidth - 16, box.maxHeight - 230);
              final code = frame == null
                  ? const SizedBox.shrink()
                  // Showing a transfer is an interactive process: the outline travels.
                  : RainbowGlow(
                      animate: true,
                      radius: 4,
                      child: QrMatrix(qr: frame, size: math.max(side, 120)),
                    );
              return landscape
                  ? Row(
                      children: [
                        Expanded(child: Center(child: code)),
                        SizedBox(
                          width: box.maxWidth * 0.36,
                          child: SingleChildScrollView(
                            padding: const EdgeInsets.all(8),
                            child: controls,
                          ),
                        ),
                      ],
                    )
                  : SingleChildScrollView(
                      padding: const EdgeInsets.symmetric(vertical: 8),
                      child: Column(
                        children: [
                          Center(child: code),
                          const SizedBox(height: 12),
                          Padding(
                            padding: const EdgeInsets.symmetric(horizontal: 16),
                            child: controls,
                          ),
                        ],
                      ),
                    );
            },
          ),
        ),
      ),
    );
  }
}

class _ShareSendScreen extends StatelessWidget {
  const _ShareSendScreen({required this.text});

  final String text;

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return Scaffold(
      appBar: AppBar(title: Text(l.shareTitle)),
      body: ListView(
        padding: const EdgeInsets.all(24),
        children: [
          Text(l.shareHint),
          const SizedBox(height: 16),
          Container(
            padding: const EdgeInsets.all(12),
            decoration: BoxDecoration(
              color: Theme.of(context).colorScheme.surfaceContainerHighest,
              borderRadius: BorderRadius.circular(8),
            ),
            child: SelectableText(
              text,
              maxLines: 8,
              style: const TextStyle(fontFamily: 'monospace', fontSize: 12),
            ),
          ),
          const SizedBox(height: 16),
          Wrap(
            spacing: 8,
            runSpacing: 8,
            children: [
              OutlinedButton.icon(
                icon: const Icon(Icons.copy),
                label: Text(l.copyText),
                onPressed: () async {
                  await Clipboard.setData(ClipboardData(text: text));
                  if (context.mounted) {
                    showSnack(context, l.copied);
                    Navigator.pop(context, true);
                  }
                },
              ),
              // Straight to Telegram when it is installed: one step less than the
              // share sheet; Telegram sees the same as through the sheet.
              FutureBuilder<String?>(
                future: PlatformServices.telegramApp(),
                builder: (context, app) => app.data == null
                    ? const SizedBox.shrink()
                    : OutlinedButton.icon(
                        icon: const Icon(Icons.send_outlined),
                        label: const Text('Telegram'),
                        onPressed: () async {
                          final ok = await PlatformServices.shareTo(
                            app.data!,
                            text,
                          );
                          if (ok && context.mounted) {
                            Navigator.pop(context, true);
                          }
                        },
                      ),
              ),
              if (Platform.isAndroid || Platform.isIOS)
                FilledButton.icon(
                  icon: const Icon(Icons.share),
                  label: Text(l.shareText),
                  onPressed: () async {
                    final result = await SharePlus.instance.share(
                      ShareParams(text: text),
                    );
                    if (context.mounted) {
                      Navigator.pop(
                        context,
                        result.status != ShareResultStatus.dismissed,
                      );
                    }
                  },
                ),
            ],
          ),
        ],
      ),
    );
  }
}

/// P5: shows the exact text that will leave MEM unencrypted, and only then offers the
/// share sheet. Cancelling sends nothing.
class _PlainSendScreen extends StatelessWidget {
  const _PlainSendScreen({required this.text});

  final String text;

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final scheme = Theme.of(context).colorScheme;
    return Scaffold(
      appBar: AppBar(title: Text(l.plainTitle)),
      body: ListView(
        padding: const EdgeInsets.all(24),
        children: [
          Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Icon(Icons.lock_open, color: scheme.error),
              const SizedBox(width: 8),
              Expanded(child: Text(l.plainWarning)),
            ],
          ),
          const SizedBox(height: 16),
          Container(
            padding: const EdgeInsets.all(12),
            decoration: BoxDecoration(
              border: Border.all(color: scheme.error),
              borderRadius: BorderRadius.circular(8),
            ),
            child: SelectableText(text),
          ),
          const SizedBox(height: 24),
          Wrap(
            alignment: WrapAlignment.end,
            spacing: 8,
            children: [
              TextButton(
                onPressed: () => Navigator.pop(context, false),
                child: Text(l.dontSend),
              ),
              FilledButton.icon(
                style: FilledButton.styleFrom(backgroundColor: scheme.error),
                icon: const Icon(Icons.share),
                label: Text(l.plainSend),
                onPressed: () async {
                  final result = await SharePlus.instance.share(
                    ShareParams(text: text),
                  );
                  if (context.mounted) {
                    Navigator.pop(
                      context,
                      result.status != ShareResultStatus.dismissed,
                    );
                  }
                },
              ),
            ],
          ),
        ],
      ),
    );
  }
}
