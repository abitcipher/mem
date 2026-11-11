import 'dart:io';

import 'package:flutter/material.dart';

import '../app_scope.dart';
import '../l10n/gen/app_localizations.dart';
import '../src/platform.dart';
import '../src/rust/api.dart';
import '../src/ui_settings.dart';
import '../theme.dart';
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
    final small = Theme.of(context).textTheme.bodySmall;
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
          Text(l.badgeDetail),
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
          const SizedBox(height: 4),
          Text(l.badgeDetailHelp, style: small),

          _Section(l.sectionQr),
          Text(l.qrPaceTitle),
          const SizedBox(height: 8),
          SegmentedButton<QrPace>(
            showSelectedIcon: false,
            segments: [
              ButtonSegment(value: QrPace.easy, label: Text(l.qrPaceEasy)),
              ButtonSegment(value: QrPace.normal, label: Text(l.qrPaceNormal)),
              ButtonSegment(value: QrPace.fast, label: Text(l.qrPaceFast)),
            ],
            selected: {s.qrPace},
            onSelectionChanged: (v) =>
                state.updateSettings((s) => s.qrPace = v.first),
          ),
          const SizedBox(height: 4),
          Text(l.qrPaceHint, style: small),

          _Section(l.sectionPrivacy),
          Text(l.defaultFloor),
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
                if (context.mounted) showSnack(context, reasonText(context, e));
              }
              state.changed();
            },
          ),
          Text(l.defaultFloorHelp, style: small),
          // Off by default; advertising the intro tag lets card holders notice us.
          SwitchListTile(
            contentPadding: EdgeInsets.zero,
            title: Text(l.requestsNearby),
            subtitle: Text(l.requestsNearbyHelp),
            value: state.node.acceptsRequestsNearby(),
            onChanged: (v) async {
              try {
                await state.node.setAcceptsRequestsNearby(enabled: v);
              } on Object catch (e) {
                if (context.mounted) showSnack(context, reasonText(context, e));
              }
              state.changed();
            },
          ),
          if (Platform.isAndroid)
            SwitchListTile(
              contentPadding: EdgeInsets.zero,
              title: Text(l.blockScreenshots),
              subtitle: Text(l.blockScreenshotsHelp),
              value: s.blockScreenshots,
              onChanged: (v) =>
                  state.updateSettings((s) => s.blockScreenshots = v),
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
            SwitchListTile(
              contentPadding: EdgeInsets.zero,
              title: Text(l.stayReachable),
              subtitle: Text(l.stayReachableHelp),
              value: s.stayReachable,
              onChanged: (v) =>
                  state.updateSettings((s) => s.stayReachable = v),
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
        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          title: Text(l.courierMode),
          subtitle: Text(l.courierModeHelp),
          value: on,
          onChanged: (v) => _set(v ? status.defaultBytes.toInt() : 0),
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
        Text(l.retryTitle),
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
        Text(l.retryHelp, style: Theme.of(context).textTheme.bodySmall),
      ],
    );
  }
}
