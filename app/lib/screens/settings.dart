import 'dart:io';

import 'package:material_ui/material_ui.dart';
import 'package:mem_calls/mem_calls.dart';
import 'package:mem_video_player/mem_video_player.dart';

import '../app_scope.dart';
import '../calls/call_controller.dart';
import '../l10n/gen/app_localizations.dart';
import '../src/platform.dart';
import '../src/rust/api.dart';
import '../src/ui_settings.dart';
import '../theme.dart';
import '../widgets/grade_symbol.dart';
import '../widgets/edit_limit.dart';
import '../widgets/help.dart';
import '../widgets/logo.dart';
import 'backup.dart';
import 'channels.dart';
import 'profile.dart';

/// Every user setting in one place, each within safe bounds. Security limits stay in the core:
/// the user chooses inside them, never beyond them.
class SettingsScreen extends StatelessWidget {
  const SettingsScreen({super.key});

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final state = AppScope.of(context);
    final s = state.settings;
    final profile = state.node.profile();
    void push(Widget w) =>
        Navigator.of(context).push(MaterialPageRoute<void>(builder: (_) => w));

    return Scaffold(
      appBar: AppBar(title: Text(l.settings)),
      body: ListView(
        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
        children: [
          ListTile(
            contentPadding: EdgeInsets.zero,
            leading: const Icon(Icons.person_outline),
            title: Text(profile.displayName ?? l.profile),
            subtitle: Text(
              profile.fingerprint.split(' ').take(4).join(' '),
              style: const TextStyle(fontFamily: 'monospace'),
            ),
            trailing: const Icon(Icons.chevron_right),
            onTap: () => push(const ProfileScreen()),
          ),

          _Section(l.appearance),
          SegmentedButton<AppTheme>(
            showSelectedIcon: false,
            segments: [
              ButtonSegment(value: AppTheme.system, label: Text(l.themeSystem)),
              ButtonSegment(value: AppTheme.white, label: Text(l.themeWhite)),
              ButtonSegment(value: AppTheme.gray, label: Text(l.themeGray)),
              ButtonSegment(value: AppTheme.black, label: Text(l.themeBlack)),
            ],
            selected: {state.theme},
            onSelectionChanged: (v) => state.theme = v.first,
          ),
          const SizedBox(height: 16),
          Text(l.language),
          const SizedBox(height: 8),
          SegmentedButton<AppLanguage>(
            showSelectedIcon: false,
            segments: [
              ButtonSegment(
                value: AppLanguage.system,
                label: Text(l.followSystem),
              ),
              // Each language is named in itself, so it can be found from either.
              const ButtonSegment(
                value: AppLanguage.en,
                label: Text('English'),
              ),
              const ButtonSegment(
                value: AppLanguage.uk,
                label: Text('Українська'),
              ),
            ],
            selected: {s.language},
            onSelectionChanged: (v) =>
                state.updateSettings((s) => s.language = v.first),
          ),
          const SizedBox(height: 8),
          const _TextSize(),

          _Section(l.sectionMessages),
          const _RetrySetting(),
          SwitchListTile(
            contentPadding: EdgeInsets.zero,
            title: Text(l.showMessageTimes),
            value: s.showMessageTimes,
            onChanged: (v) =>
                state.updateSettings((s) => s.showMessageTimes = v),
          ),
          const _BadgeSettings(),
          Explained(
            help: l.editLimitHelp,
            builder: (context, help) => Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                titleWithHelp(l.editLimitTitle, help),
                EditLimitSlider(
                  position: state.node.editLimit(),
                  onChanged: (p) async {
                    await state.node.setEditLimit(position: p);
                    state.changed();
                  },
                ),
              ],
            ),
          ),
          if (CallController.supported)
            Explained(
              help: l.voiceCallsHelp,
              builder: (context, help) => SwitchListTile(
                contentPadding: EdgeInsets.zero,
                title: titleWithHelp(l.voiceCalls, help),
                value: s.voiceCalls,
                onChanged: (v) => state.updateSettings((s) => s.voiceCalls = v),
              ),
            ),
          if (CallController.supported && s.voiceCalls) ...[
            const _CallTones(),
            const _CallTimesSetting(),
            SwitchListTile(
              contentPadding: EdgeInsets.zero,
              title: Text(l.callHistory),
              subtitle: Text(l.callHistoryHelp),
              value: s.callHistory,
              onChanged: (v) => state.updateSettings((s) => s.callHistory = v),
            ),
          ],
          // Only where the platform has an engine; elsewhere files always open in another app.
          if (MemVideoPlayer.isSupported)
            Explained(
              help: l.mediaPlayerHelp,
              builder: (context, help) => SwitchListTile(
                contentPadding: EdgeInsets.zero,
                title: titleWithHelp(l.mediaPlayer, help),
                value: s.builtInPlayer,
                onChanged: (v) =>
                    state.updateSettings((s) => s.builtInPlayer = v),
              ),
            ),

          _Section(l.sectionQr),
          Explained(
            help: l.qrPaceHint,
            builder: (context, help) => Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                titleWithHelp(l.qrPaceTitle, help),
                const SizedBox(height: 8),
                SegmentedButton<QrPace>(
                  showSelectedIcon: false,
                  segments: [
                    ButtonSegment(
                      value: QrPace.easy,
                      label: Text(l.qrPaceEasy),
                    ),
                    ButtonSegment(
                      value: QrPace.normal,
                      label: Text(l.qrPaceNormal),
                    ),
                    ButtonSegment(
                      value: QrPace.fast,
                      label: Text(l.qrPaceFast),
                    ),
                  ],
                  selected: {s.qrPace},
                  onSelectionChanged: (v) =>
                      state.updateSettings((s) => s.qrPace = v.first),
                ),
              ],
            ),
          ),

          _Section(l.sectionPrivacy),
          Explained(
            help: l.defaultFloorHelp,
            builder: (context, help) => Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                titleWithHelp(l.defaultFloor, help),
                DropdownButton<int>(
                  isExpanded: true,
                  value: state.node.defaultFloor(),
                  items: [
                    for (var g = 0; g <= 4; g++)
                      DropdownMenuItem(value: g, child: Text(gradeLabel(l, g))),
                  ],
                  onChanged: (g) async {
                    if (g == null) return;
                    try {
                      await state.node.setDefaultFloor(floor: g);
                    } on Object catch (e) {
                      if (context.mounted) {
                        showSnack(context, reasonText(context, e));
                      }
                    }
                    state.changed();
                  },
                ),
              ],
            ),
          ),
          // Off by default; advertising the intro tag lets card holders notice us.
          Explained(
            help: l.requestsNearbyHelp,
            builder: (context, help) => SwitchListTile(
              contentPadding: EdgeInsets.zero,
              title: titleWithHelp(l.requestsNearby, help),
              value: state.node.acceptsRequestsNearby(),
              onChanged: (v) async {
                try {
                  await state.node.setAcceptsRequestsNearby(enabled: v);
                } on Object catch (e) {
                  if (context.mounted) {
                    showSnack(context, reasonText(context, e));
                  }
                }
                state.changed();
              },
            ),
          ),
          if (Platform.isAndroid)
            Explained(
              help: l.blockScreenshotsHelp,
              builder: (context, help) => SwitchListTile(
                contentPadding: EdgeInsets.zero,
                title: titleWithHelp(l.blockScreenshots, help),
                value: s.blockScreenshots,
                onChanged: (v) =>
                    state.updateSettings((s) => s.blockScreenshots = v),
              ),
            ),

          _Section(l.sectionCourier),
          const _CourierSetting(),

          _Section(l.channels),
          SwitchListTile(
            contentPadding: EdgeInsets.zero,
            title: Text(l.showNetworkStatus),
            value: s.showNetworkStatus,
            onChanged: (v) =>
                state.updateSettings((s) => s.showNetworkStatus = v),
          ),
          if (Platform.isAndroid) ...[
            Explained(
              help: l.stayReachableHelp,
              builder: (context, help) => SwitchListTile(
                contentPadding: EdgeInsets.zero,
                title: titleWithHelp(l.stayReachable, help),
                value: s.stayReachable,
                onChanged: (v) =>
                    state.updateSettings((s) => s.stayReachable = v),
              ),
            ),
            if (s.stayReachable) const _BatteryStatus(),
          ],
          ListTile(
            contentPadding: EdgeInsets.zero,
            leading: const Icon(Icons.hub_outlined),
            title: Text(l.channels),
            trailing: const Icon(Icons.chevron_right),
            onTap: () => push(const ChannelsScreen()),
          ),

          _Section(l.backupTitle),
          ListTile(
            contentPadding: EdgeInsets.zero,
            leading: const Icon(Icons.backup_outlined),
            title: Text(l.backupTitle),
            trailing: const Icon(Icons.chevron_right),
            onTap: () => push(const BackupScreen()),
          ),

          const Divider(height: 40),
          Row(
            children: [
              const MemLogo(size: 40),
              const SizedBox(width: 12),
              Text(
                l.about,
                style: const TextStyle(fontWeight: FontWeight.bold),
              ),
            ],
          ),
          const SizedBox(height: 8),
          Text(l.aboutBody),
          const SizedBox(height: 24),
        ],
      ),
    );
  }
}

/// Courier mode: carry sealed messages for other people, within a storage limit.
class _CourierSetting extends StatefulWidget {
  const _CourierSetting();

  @override
  State<_CourierSetting> createState() => _CourierSettingState();
}

class _CourierSettingState extends State<_CourierSetting> {
  /// Storage choices in MiB; the slider moves between them.
  static const _steps = [8, 16, 32, 64, 128, 256, 512];
  static const _mib = 1024 * 1024;
  double? _dragging;

  int _stepFor(int bytes) {
    final mib = bytes ~/ _mib;
    var best = 0;
    for (var i = 0; i < _steps.length; i++) {
      if ((_steps[i] - mib).abs() < (_steps[best] - mib).abs()) best = i;
    }
    return best;
  }

  Future<void> _set(int bytes) async {
    final state = AppScope.read(context);
    await state.node.setCourierLimit(bytes: BigInt.from(bytes));
    state.changed();
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final status = AppScope.of(context).node.courierStatus();
    final limit = status.limitBytes.toInt();
    final on = limit > 0;
    final step = (_dragging ?? _stepFor(limit).toDouble()).round();
    String mb(int bytes) =>
        (bytes / _mib).toStringAsFixed(bytes < 10 * _mib ? 1 : 0);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Explained(
          help: l.courierModeHelp,
          builder: (context, help) => SwitchListTile(
            contentPadding: EdgeInsets.zero,
            title: titleWithHelp(l.courierMode, help),
            value: on,
            onChanged: (v) => _set(v ? status.defaultBytes.toInt() : 0),
          ),
        ),
        if (on) ...[
          Text(l.courierLimit(_steps[step])),
          Slider(
            min: 0,
            max: (_steps.length - 1).toDouble(),
            divisions: _steps.length - 1,
            value: step.toDouble(),
            label: l.courierLimit(_steps[step]),
            onChanged: (v) => setState(() => _dragging = v),
            onChangeEnd: (v) {
              setState(() => _dragging = null);
              _set(_steps[v.round()] * _mib);
            },
          ),
          Text(
            l.courierUsage(
              status.bundles,
              mb(status.usedBytes.toInt()),
              mb(limit),
            ),
            style: Theme.of(context).textTheme.bodySmall,
          ),
        ],
      ],
    );
  }
}

/// Whether Android exempts MEM from battery optimization; re-checked when the app returns from
/// the system dialog.
class _BatteryStatus extends StatefulWidget {
  const _BatteryStatus();

  @override
  State<_BatteryStatus> createState() => _BatteryStatusState();
}

class _BatteryStatusState extends State<_BatteryStatus>
    with WidgetsBindingObserver {
  bool? _unrestricted;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    _check();
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    super.dispose();
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.resumed) _check();
  }

  Future<void> _check() async {
    final v = await PlatformServices.batteryUnrestricted();
    if (mounted) setState(() => _unrestricted = v);
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final ok = _unrestricted;
    if (ok == null) return const SizedBox.shrink();
    return ListTile(
      contentPadding: EdgeInsets.zero,
      leading: Icon(
        ok ? Icons.battery_full : Icons.battery_alert,
        color: ok ? null : MemColors.limited(context),
      ),
      title: Text(ok ? l.batteryUnrestricted : l.batteryRestricted),
      onTap: ok ? null : PlatformServices.requestBatteryUnrestricted,
    );
  }
}

class _Section extends StatelessWidget {
  const _Section(this.title);

  final String title;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.only(top: 28, bottom: 8),
    child: Text(
      title,
      style: Theme.of(context).textTheme.titleSmall
          ?.copyWith(fontWeight: FontWeight.bold),
    ),
  );
}

/// Text size: the system's, or a fixed size within bounds.
class _TextSize extends StatefulWidget {
  const _TextSize();

  @override
  State<_TextSize> createState() => _TextSizeState();
}

class _TextSizeState extends State<_TextSize> {
  double? _dragging;

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final state = AppScope.of(context);
    final percent = state.settings.textPercent;
    const min = UiSettings.minTextPercent;
    const max = UiSettings.maxTextPercent;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          title: Text(l.textSize),
          subtitle: percent == null ? null : Text(l.percent(percent)),
          value: percent == null,
          onChanged: (follow) =>
              state.updateSettings((s) => s.textPercent = follow ? null : 100),
        ),
        if (percent != null)
          Slider(
            min: min.toDouble(),
            max: max.toDouble(),
            divisions: (max - min) ~/ 5,
            value: (_dragging ?? percent.toDouble()).clamp(
              min.toDouble(),
              max.toDouble(),
            ),
            label: l.percent((_dragging ?? percent).round()),
            onChanged: (v) => setState(() => _dragging = v),
            onChangeEnd: (v) {
              setState(() => _dragging = null);
              state.updateSettings((s) => s.textPercent = v.round());
            },
          ),
      ],
    );
  }
}

/// How long unconfirmed messages are retried: a bounded user setting.
class _RetrySetting extends StatefulWidget {
  const _RetrySetting();

  @override
  State<_RetrySetting> createState() => _RetrySettingState();
}

class _RetrySettingState extends State<_RetrySetting> {
  double? _days;

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final state = AppScope.of(context);
    final range = state.node.retryDays();
    final min = range.min.toDouble();
    final max = range.max.toDouble();
    final days = (_days ?? range.current.toDouble()).clamp(min, max);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Explained(
          help: l.retryHelp,
          builder: (context, help) => Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              titleWithHelp(l.retryTitle, help),
              Row(
                children: [
                  Expanded(
                    child: Slider(
                      min: min,
                      max: max,
                      divisions: range.max - range.min,
                      value: days,
                      label: l.retryDays(days.round()),
                      onChanged: (v) => setState(() => _days = v),
                      onChangeEnd: (v) async {
                        await state.node.setRetryDays(days: v.round());
                        state.changed();
                      },
                    ),
                  ),
                  Text(l.retryDays(days.round())),
                ],
              ),
            ],
          ),
        ),
      ],
    );
  }
}

/// What the message badges slider does at [mode], in words.
String badgeModeText(AppLocalizations l, int mode) {
  if (mode <= UiSettings.minBadgeMode) return l.badgeModeMinimal;
  if (mode == UiSettings.minBadgeMode + 1) return l.badgeModeSymbols;
  final d = UiSettings.badgeDelay(mode);
  if (d == null) return l.badgeModeFull;
  final s = d.inSeconds;
  final text = s < 60
      ? l.durationSeconds(s)
      : s < 3600
      ? l.durationMinutes(s ~/ 60)
      : l.durationHours(s ~/ 3600);
  return l.badgeModeCollapse(text);
}

/// Message badges: the slider, the symbol style with a preview, and the detail of full badges.
class _BadgeSettings extends StatefulWidget {
  const _BadgeSettings();

  @override
  State<_BadgeSettings> createState() => _BadgeSettingsState();
}

class _BadgeSettingsState extends State<_BadgeSettings> {
  double? _dragging;

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final state = AppScope.of(context);
    final s = state.settings;
    final small = Theme.of(context).textTheme.bodySmall;
    final mode = (_dragging ?? s.badgeMode.toDouble()).round();
    final surface = Theme.of(context).colorScheme.surface;
    final digit = Theme.of(context).textTheme.labelSmall?.fontSize ?? 11;
    Widget symbol(String grade, SymbolShape shape) => GradeSymbol(
      grade: grade,
      shape: shape,
      background: surface,
      fontSize: digit * 1.6,
    );
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Explained(
          help: l.messageBadgesHelp,
          builder: (context, help) => Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              titleWithHelp(l.messageBadges, help),
              Slider(
                min: UiSettings.minBadgeMode.toDouble(),
                max: UiSettings.maxBadgeMode.toDouble(),
                divisions: UiSettings.maxBadgeMode - UiSettings.minBadgeMode,
                value: mode.toDouble(),
                label: badgeModeText(l, mode),
                onChanged: (v) => setState(() => _dragging = v),
                onChangeEnd: (v) {
                  setState(() => _dragging = null);
                  state.updateSettings((s) => s.badgeMode = v.round());
                },
              ),
              Text(badgeModeText(l, mode)),
            ],
          ),
        ),
        const SizedBox(height: 12),
        Explained(
          help: l.badgeSymbolsHelp,
          builder: (context, help) => Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              titleWithHelp(l.badgeSymbols, help),
              const SizedBox(height: 8),
              SegmentedButton<SymbolStyle>(
                showSelectedIcon: false,
                segments: [
                  ButtonSegment(
                    value: SymbolStyle.byGrade,
                    label: Text(l.badgeSymbolsByGrade),
                  ),
                  ButtonSegment(
                    value: SymbolStyle.oneShape,
                    label: Text(l.badgeSymbolsOneShape),
                  ),
                ],
                selected: {s.symbolStyle},
                onSelectionChanged: (v) =>
                    state.updateSettings((s) => s.symbolStyle = v.first),
              ),
              if (s.symbolStyle == SymbolStyle.oneShape) ...[
                const SizedBox(height: 8),
                Wrap(
                  spacing: 8,
                  children: [
                    for (final shape in oneShapeChoices)
                      ChoiceChip(
                        label: symbol('P1', shape),
                        selected: s.symbolShape == shape,
                        onSelected: (_) =>
                            state.updateSettings((s) => s.symbolShape = shape),
                      ),
                  ],
                ),
              ],
              const SizedBox(height: 8),
              // Preview: every grade as it will look.
              Wrap(
                spacing: 14,
                children: [
                  for (final g in const ['P0', 'P1', 'P2', 'P3', 'P4', 'P5'])
                    Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        symbol(g, shapeFor(g, s.symbolStyle, s.symbolShape)),
                        const SizedBox(width: 3),
                        Text(g, style: small),
                      ],
                    ),
                ],
              ),
            ],
          ),
        ),
        const SizedBox(height: 12),
        Explained(
          help: l.badgeDetailHelp,
          builder: (context, help) => Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              titleWithHelp(l.badgeDetail, help),
              const SizedBox(height: 8),
              SegmentedButton<BadgeDetail>(
                showSelectedIcon: false,
                segments: [
                  ButtonSegment(
                    value: BadgeDetail.compact,
                    label: Text(l.badgeCompact),
                  ),
                  ButtonSegment(
                    value: BadgeDetail.detailed,
                    label: Text(l.badgeDetailed),
                  ),
                ],
                selected: {s.badgeDetail},
                onSelectionChanged: (v) =>
                    state.updateSettings((s) => s.badgeDetail = v.first),
              ),
            ],
          ),
        ),
      ],
    );
  }
}

/// The incoming tone and the ringback, each with its volume and a preview.
class _CallTones extends StatefulWidget {
  const _CallTones();

  @override
  State<_CallTones> createState() => _CallTonesState();
}

class _CallTonesState extends State<_CallTones> {
  final _preview = MemTonePlayer();

  @override
  void dispose() {
    _preview.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final state = AppScope.of(context);
    final s = state.settings;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _toneRow(
          context,
          title: l.toneIncoming,
          tones: MemTone.forIncoming,
          value: s.incomingTone,
          volume: s.incomingVolume,
          onTone: (v) => state.updateSettings((s) => s.incomingTone = v),
          onVolume: (v) => state.updateSettings((s) => s.incomingVolume = v),
        ),
        _toneRow(
          context,
          title: l.toneOutgoing,
          tones: MemTone.forOutgoing,
          value: s.outgoingTone,
          volume: s.outgoingVolume,
          onTone: (v) => state.updateSettings((s) => s.outgoingTone = v),
          onVolume: (v) => state.updateSettings((s) => s.outgoingVolume = v),
        ),
      ],
    );
  }

  Widget _toneRow(
    BuildContext context, {
    required String title,
    required List<MemTone> tones,
    required String value,
    required double volume,
    required void Function(String) onTone,
    required void Function(double) onVolume,
  }) {
    final l = AppLocalizations.of(context);
    final tone = MemTone.byId(value);
    return Padding(
      padding: const EdgeInsets.only(top: 8),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Text(title),
          Row(
            children: [
              Expanded(
                child: DropdownButton<String>(
                  isExpanded: true,
                  value: tone?.id ?? 'off',
                  items: [
                    for (final t in tones)
                      DropdownMenuItem(
                        value: t.id,
                        child: Text(toneName(l, t)),
                      ),
                    DropdownMenuItem(value: 'off', child: Text(l.toneOff)),
                  ],
                  onChanged: (v) {
                    if (v != null) onTone(v);
                  },
                ),
              ),
              IconButton(
                tooltip: l.tonePreview,
                icon: const Icon(Icons.play_arrow),
                onPressed: tone == null
                    ? null
                    : () => _preview.play(tone, volume: volume, loop: false),
              ),
            ],
          ),
          Row(
            children: [
              const Icon(Icons.volume_down, size: 20),
              Expanded(
                child: Slider(
                  value: volume,
                  label: '${(volume * 100).round()}%',
                  divisions: 10,
                  onChanged: tone == null ? null : onVolume,
                ),
              ),
              const Icon(Icons.volume_up, size: 20),
            ],
          ),
        ],
      ),
    );
  }
}

/// A tone's name in the app's language.
String toneName(AppLocalizations l, MemTone t) => switch (t) {
  MemTone.chime => l.toneChime,
  MemTone.classic => l.toneClassic,
  MemTone.pulse => l.tonePulse,
  MemTone.soft => l.toneSoft,
  MemTone.ringback => l.toneRingback,
  MemTone.beep => l.toneBeep,
  // Fixed cues, never offered in the list.
  MemTone.dial => l.callConnecting,
  MemTone.unavailable => l.callUnreachable,
};

/// The call times: how long a call waits to reach the other phone, rings, and waits for a
/// lost connection. Each within hard bounds.
class _CallTimesSetting extends StatelessWidget {
  const _CallTimesSetting();

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final state = AppScope.of(context);
    final s = state.settings;
    Widget row(
      String title,
      String help,
      CallTime bounds,
      int value,
      void Function(UiSettings s, int v) set,
    ) {
      final step = bounds.max - bounds.min > 120 ? 15 : 5;
      return Explained(
        help: help,
        builder: (context, helpText) => Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Expanded(child: titleWithHelp(title, helpText)),
                Text(l.callSeconds(value)),
              ],
            ),
            Slider(
              min: bounds.min.toDouble(),
              max: bounds.max.toDouble(),
              divisions: (bounds.max - bounds.min) ~/ step,
              value: value.toDouble(),
              label: l.callSeconds(value),
              onChanged: (v) =>
                  state.updateSettings((s) => set(s, bounds.clamp(v.round()))),
            ),
          ],
        ),
      );
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        row(
          l.callReachTitle,
          l.callReachHelp,
          CallTimes.reach,
          s.callReachSeconds,
          (s, v) => s.callReachSeconds = v,
        ),
        row(
          l.callRingTitle,
          l.callRingHelp,
          CallTimes.ring,
          s.callRingSeconds,
          (s, v) => s.callRingSeconds = v,
        ),
        row(
          l.callReconnectTitle,
          l.callReconnectHelp,
          CallTimes.reconnect,
          s.callReconnectSeconds,
          (s, v) => s.callReconnectSeconds = v,
        ),
      ],
    );
  }
}
