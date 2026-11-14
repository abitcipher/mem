import 'package:material_ui/material_ui.dart';
import 'package:mem_push/mem_push.dart';
import 'package:mem_radio/mem_radio.dart';

import '../app_scope.dart';
import '../l10n/gen/app_localizations.dart';
import '../src/ble.dart';
import '../src/wifi_direct.dart';
import '../src/platform.dart';
import '../src/push.dart';
import '../src/rust/api.dart';
import '../widgets/attachments.dart' show formatSize;
import '../widgets/grade_badge.dart';
import '../theme.dart';
import 'network_map.dart';

/// Every channel with the exposure profile the core reports for it. The text comes from the core's disclosure generator; this screen only styles it.
class ChannelsScreen extends StatelessWidget {
  const ChannelsScreen({super.key});

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final state = AppScope.of(context);
    final channels = state.node.channels(locale: localeCode(context));
    return Scaffold(
      appBar: AppBar(
        title: Text(l.channels),
        actions: [
          IconButton(
            tooltip: l.netmapTitle,
            icon: const Icon(Icons.hub_outlined),
            onPressed: () => Navigator.of(context).push(
              MaterialPageRoute<void>(builder: (_) => const NetworkMapScreen()),
            ),
          ),
        ],
      ),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          Text(l.channelsIntro),
          const SizedBox(height: 8),
          for (final c in channels) ChannelCard(info: c),
        ],
      ),
    );
  }
}

/// One channel: its name, grade, who can see what, and its controls (also shown from the
/// network map, so both show the same disclosure and switch).
class ChannelCard extends StatelessWidget {
  /// Creates the card.
  const ChannelCard({required this.info, super.key});

  final ChannelInfo info;

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final state = AppScope.read(context);
    Widget line(String text, IconData icon, Color color) => Padding(
      padding: const EdgeInsets.only(top: 4),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Icon(icon, size: 16, color: color),
          const SizedBox(width: 6),
          Expanded(child: Text(text)),
        ],
      ),
    );
    return Card(
      margin: const EdgeInsets.only(top: 12),
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Icon(channelIcon(info.channel)),
                const SizedBox(width: 8),
                Expanded(
                  child: Text(
                    info.name,
                    style: Theme.of(context).textTheme.titleMedium,
                  ),
                ),
                GradeBadge(
                  label: channelLabel(l, info.channel),
                  grade: info.grade,
                ),
              ],
            ),
            const SizedBox(height: 8),
            Text(
              l.whoCanSee(info.observers),
              style: const TextStyle(fontStyle: FontStyle.italic),
            ),
            for (final e in info.exposed)
              line(e, Icons.visibility, MemColors.exposed(context)),
            for (final e in info.limited)
              line(e, Icons.visibility_outlined, MemColors.limited(context)),
            for (final e in info.protected)
              line(e, Icons.lock, MemColors.protected(context)),
            if (info.canDisable && info.channel == Channel.ble) ...[
              const Divider(height: 24),
              _BleSwitch(enabled: info.enabled),
            ] else if (info.canDisable &&
                info.channel == Channel.wifiDirect) ...[
              const Divider(height: 24),
              _WifiDirectSwitch(enabled: info.enabled),
            ] else if (info.canDisable && info.channel == Channel.email) ...[
              const Divider(height: 24),
              _EmailSettings(enabled: info.enabled),
            ] else if (info.channel == Channel.deviceHub) ...[
              const Divider(height: 24),
              const _DeviceHubServe(),
            ] else if (info.canDisable && info.channel == Channel.hub) ...[
              const Divider(height: 24),
              _HubSettings(enabled: info.enabled),
            ] else if (info.canDisable) ...[
              const Divider(height: 24),
              SwitchListTile(
                contentPadding: EdgeInsets.zero,
                title: Text(l.lanToggle),
                subtitle: Text(l.lanToggleHelp),
                value: info.enabled,
                onChanged: (on) async {
                  await state.node.setLanEnabled(enabled: on);
                  state.changed();
                },
              ),
            ],
          ],
        ),
      ),
    );
  }
}

/// Wi-Fi Direct on or off. Problems show as a line of text with an icon and, where
/// possible, a button that fixes them; no dialogs.
class _WifiDirectSwitch extends StatefulWidget {
  const _WifiDirectSwitch({required this.enabled});

  final bool enabled;

  @override
  State<_WifiDirectSwitch> createState() => _WifiDirectSwitchState();
}

class _WifiDirectSwitchState extends State<_WifiDirectSwitch>
    with WidgetsBindingObserver {
  WifiStatus? _status;

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
    if (!WifiDirectBridge.available) return;
    final status = await MemRadio.wifiStatus();
    if (!mounted) return;
    setState(() => _status = status);
    await AppScope.read(context).wifi.sync();
  }

  Future<void> _turn(bool on) async {
    final state = AppScope.read(context);
    if (on && WifiDirectBridge.available) {
      final status = await MemRadio.wifiStatus();
      if (!status.permissions) await MemRadio.wifiRequestPermissions();
    }
    await state.node.setWifiDirectEnabled(enabled: on);
    state.changed();
    await _check();
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final s = _status;
    (String, String?, Future<void> Function()?)? problem;
    if (widget.enabled && s != null) {
      if (!s.supported) {
        problem = (l.wifiDirectUnsupported, null, null);
      } else if (!s.permissions) {
        problem = (
          l.wifiDirectPermission,
          l.allow,
          () async {
            await MemRadio.wifiRequestPermissions();
            await _check();
          },
        );
      } else if (s.locationNeeded && !s.locationOn) {
        problem = (
          l.wifiDirectLocation,
          l.bleOpenSettings,
          MemRadio.openLocationSettings,
        );
      }
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          title: Text(l.wifiDirectToggle),
          subtitle: Text(l.wifiDirectToggleHelp),
          value: widget.enabled,
          onChanged: _turn,
        ),
        if (problem != null)
          Row(
            children: [
              Icon(
                Icons.error_outline,
                size: 16,
                color: Theme.of(context).colorScheme.error,
              ),
              const SizedBox(width: 6),
              Expanded(child: Text(problem.$1)),
              if (problem.$2 != null)
                TextButton(onPressed: problem.$3, child: Text(problem.$2!)),
            ],
          ),
      ],
    );
  }
}

/// Bluetooth on or off. Turning it on asks for the permissions Android needs,
/// saying first that MEM never reads a location; the line below says what still stops it.
class _BleSwitch extends StatefulWidget {
  const _BleSwitch({required this.enabled});

  final bool enabled;

  @override
  State<_BleSwitch> createState() => _BleSwitchState();
}

class _BleSwitchState extends State<_BleSwitch> with WidgetsBindingObserver {
  RadioStatus? _status;

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

  // Back from the system settings or a permission dialog.
  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.resumed) _check();
  }

  Future<void> _check() async {
    if (!BleBridge.available) return;
    final status = await MemRadio.status();
    if (!mounted) return;
    setState(() => _status = status);
    await AppScope.read(context).ble.sync();
  }

  Future<void> _turn(bool on) async {
    final state = AppScope.read(context);
    final l = AppLocalizations.of(context);
    if (on && BleBridge.available) {
      var status = await MemRadio.status();
      if (!status.permissions) {
        if (status.locationNeeded && mounted) {
          final go = await showDialog<bool>(
            context: context,
            builder: (context) => AlertDialog(
              content: Text(l.bleLocationExplain),
              actions: [
                TextButton(
                  onPressed: () => Navigator.pop(context, false),
                  child: Text(l.cancel),
                ),
                FilledButton(
                  onPressed: () => Navigator.pop(context, true),
                  child: Text(l.allow),
                ),
              ],
            ),
          );
          if (go != true) return;
        }
        if (!await MemRadio.requestPermissions()) {
          await _check();
          return;
        }
        status = await MemRadio.status();
      }
      if (!status.bluetoothOn) await MemRadio.enableBluetooth();
    }
    await state.node.setBleEnabled(enabled: on);
    state.changed();
    await _check();
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final s = _status;
    final ble = AppScope.of(context).ble;
    (String, String?, Future<void> Function()?)? problem;
    if (widget.enabled && s != null) {
      if (!s.supported) {
        problem = (l.bleStatusUnsupported, null, null);
      } else if (!s.permissions) {
        problem = (
          l.bleStatusPermission,
          l.allow,
          () async {
            await MemRadio.requestPermissions();
            await _check();
          },
        );
      } else if (!s.bluetoothOn) {
        problem = (l.bleStatusOff, l.bleTurnOn, MemRadio.enableBluetooth);
      } else if (s.locationNeeded && !s.locationOn) {
        problem = (
          l.bleStatusLocation,
          l.bleOpenSettings,
          MemRadio.openLocationSettings,
        );
      }
    }
    return ValueListenableBuilder(
      valueListenable: ble.radioState,
      builder: (context, radio, _) => Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SwitchListTile(
            contentPadding: EdgeInsets.zero,
            title: Text(l.bleToggle),
            subtitle: Text('${l.bleToggleHelp} ${l.bleBackgroundNote}'),
            value: widget.enabled,
            onChanged: _turn,
          ),
          if (problem != null)
            Row(
              children: [
                Icon(
                  Icons.error_outline,
                  size: 16,
                  color: Theme.of(context).colorScheme.error,
                ),
                const SizedBox(width: 6),
                Expanded(child: Text(problem.$1)),
                if (problem.$2 != null)
                  TextButton(onPressed: problem.$3, child: Text(problem.$2!)),
              ],
            )
          else if (widget.enabled && radio == RadioState.running)
            Text(
              l.bleStatusRunning,
              style: Theme.of(context).textTheme.bodySmall,
            ),
        ],
      ),
    );
  }
}

/// This device as a hub for its contacts: on or off, how much it keeps, what it holds.
class _DeviceHubServe extends StatelessWidget {
  const _DeviceHubServe();

  static const _mib = 1024 * 1024;
  static const _totals = [64, 128, 256, 512, 1024];
  static const _shares = [8, 16, 32, 64, 128];

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final state = AppScope.of(context);
    final s = state.node.deviceHubStatus();
    final total = s.totalBytes.toInt() ~/ _mib;
    final share = s.perContactBytes.toInt() ~/ _mib;
    Future<void> set({bool? serve, int? totalMib, int? shareMib}) async {
      final t = totalMib ?? total;
      await state.node.setDeviceHub(
        serve: serve ?? s.serve,
        totalBytes: BigInt.from(t * _mib),
        perContactBytes: BigInt.from((shareMib ?? share).clamp(1, t) * _mib),
      );
      state.changed();
    }

    DropdownButton<int> pick(
      List<int> steps,
      int value,
      void Function(int) on,
    ) => DropdownButton<int>(
      value: steps.contains(value) ? value : null,
      hint: Text('$value MB'),
      items: [
        for (final v in steps) DropdownMenuItem(value: v, child: Text('$v MB')),
      ],
      onChanged: s.serve ? (v) => v == null ? null : on(v) : null,
    );
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          title: Text(l.deviceHubServe),
          subtitle: Text(l.deviceHubServeHelp),
          value: s.serve,
          onChanged: (on) => set(serve: on),
        ),
        Row(
          children: [
            Expanded(child: Text(l.deviceHubSpace)),
            pick(_totals, total, (v) => set(totalMib: v)),
          ],
        ),
        Row(
          children: [
            Expanded(child: Text(l.deviceHubPerContact)),
            pick(_shares, share, (v) => set(shareMib: v)),
          ],
        ),
        if (s.bundles > BigInt.zero)
          Text(
            l.deviceHubUsage(
              s.bundles.toInt(),
              formatSize(s.usedBytes.toInt()),
            ),
          ),
      ],
    );
  }
}

/// The hub: its address, and on or off. Off until the user turns it on; there is no
/// default hub yet.
class _HubSettings extends StatefulWidget {
  const _HubSettings({required this.enabled});

  final bool enabled;

  @override
  State<_HubSettings> createState() => _HubSettingsState();
}

class _HubSettingsState extends State<_HubSettings> {
  final _url = TextEditingController();
  String? _saved;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final url = AppScope.read(context).node.hubUrl();
    if (_saved == null && url != null) _url.text = url;
    _saved = url ?? '';
  }

  @override
  void dispose() {
    _url.dispose();
    super.dispose();
  }

  Future<void> _save() async {
    final state = AppScope.read(context);
    final text = _url.text.trim();
    try {
      await state.node.setHubUrl(url: text.isEmpty ? null : text);
      _saved = text;
      state.changed();
    } on Object catch (e) {
      if (mounted) showSnack(context, reasonText(context, e));
    }
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final state = AppScope.of(context);
    final hasUrl = (state.node.hubUrl() ?? '').isNotEmpty;
    final online = state.node.hubOnline();
    final partners = state.node.hubPartnerOperators();
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            Expanded(
              child: TextField(
                controller: _url,
                keyboardType: TextInputType.url,
                autocorrect: false,
                decoration: InputDecoration(
                  labelText: l.hubUrl,
                  hintText: l.hubUrlHint,
                ),
                onChanged: (_) => setState(() {}),
              ),
            ),
            const SizedBox(width: 8),
            TextButton(
              onPressed: _url.text.trim() == _saved ? null : _save,
              child: Text(l.hubUrlSave),
            ),
          ],
        ),
        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          title: Text(l.hubToggle),
          subtitle: Text(hasUrl ? l.hubToggleHelp : l.hubNeedsUrl),
          value: widget.enabled,
          onChanged: hasUrl || widget.enabled
              ? (on) async {
                  await state.node.setHubEnabled(enabled: on);
                  state.changed();
                }
              : null,
        ),
        if (widget.enabled && hasUrl)
          Text(
            online ? l.hubOnline : l.hubOffline,
            style: Theme.of(context).textTheme.bodySmall,
          ),
        // Partner operators: shown only when the signed hub list names some.
        if (partners.isNotEmpty) ...[
          const SizedBox(height: 8),
          Text(
            l.hubPartners(partners.join(', ')),
            style: Theme.of(context).textTheme.bodySmall,
          ),
          SwitchListTile(
            contentPadding: EdgeInsets.zero,
            title: Text(l.ownOperatorOnly),
            subtitle: Text(l.ownOperatorOnlyHelp),
            value: state.node.ownOperatorOnly(),
            onChanged: (on) async {
              await state.node.setOwnOperatorOnly(enabled: on);
              state.changed();
            },
          ),
        ],
        if (PushBridge.available && widget.enabled && hasUrl)
          SwitchListTile(
            contentPadding: EdgeInsets.zero,
            title: Text(l.pushToggle),
            subtitle: Text(l.pushToggleHelp),
            value: state.settings.push,
            onChanged: (on) => _push(on: on),
          ),
      ],
    );
  }

  /// Wake-ups through UnifiedPush: pick a distributor app if there are several.
  Future<void> _push({required bool on}) async {
    final state = AppScope.read(context);
    final l = AppLocalizations.of(context);
    if (!on) {
      await state.push.enable(on: false);
      state.changed();
      return;
    }
    final apps = await MemPush.distributors();
    if (!mounted) return;
    if (apps.isEmpty) {
      showSnack(context, l.pushNoDistributor);
      return;
    }
    var pick = apps.first;
    if (apps.length > 1) {
      final chosen = await showDialog<String>(
        context: context,
        builder: (context) => SimpleDialog(
          title: Text(l.pushPick),
          children: [
            for (final a in apps)
              SimpleDialogOption(
                onPressed: () => Navigator.pop(context, a),
                child: Text(a),
              ),
          ],
        ),
      );
      if (chosen == null) return;
      pick = chosen;
    }
    await PlatformServices.askNotifications();
    await state.push.enable(on: true, distributor: pick);
    state.changed();
  }
}

/// The email bridge: the user's mail account and the switch. The app password goes to
/// the core and never comes back to the screen.
class _EmailSettings extends StatefulWidget {
  const _EmailSettings({required this.enabled});

  final bool enabled;

  @override
  State<_EmailSettings> createState() => _EmailSettingsState();
}

class _EmailSettingsState extends State<_EmailSettings> {
  final _address = TextEditingController();
  final _imap = TextEditingController();
  final _imapPort = TextEditingController(text: '993');
  final _smtp = TextEditingController();
  final _smtpPort = TextEditingController(text: '465');
  final _user = TextEditingController();
  final _password = TextEditingController();
  bool _loaded = false;
  bool _hasAccount = false;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    if (_loaded) return;
    _loaded = true;
    final a = AppScope.read(context).node.emailAccount();
    if (a != null) {
      _hasAccount = true;
      _address.text = a.address;
      _imap.text = a.imapHost;
      _imapPort.text = '${a.imapPort}';
      _smtp.text = a.smtpHost;
      _smtpPort.text = '${a.smtpPort}';
      _user.text = a.username;
    }
  }

  @override
  void dispose() {
    for (final c in [
      _address,
      _imap,
      _imapPort,
      _smtp,
      _smtpPort,
      _user,
      _password,
    ]) {
      c.dispose();
    }
    super.dispose();
  }

  Future<void> _save() async {
    final state = AppScope.read(context);
    final l = AppLocalizations.of(context);
    try {
      await state.node.setEmailAccount(
        account: EmailAccountItem(
          address: _address.text,
          imapHost: _imap.text,
          imapPort: int.tryParse(_imapPort.text) ?? 0,
          smtpHost: _smtp.text,
          smtpPort: int.tryParse(_smtpPort.text) ?? 0,
          username: _user.text.isEmpty ? _address.text : _user.text,
          password: _password.text.isEmpty ? null : _password.text,
        ),
      );
      _password.clear();
      _hasAccount = true;
      state.changed();
      if (mounted) showSnack(context, l.emailSaved);
    } on Object catch (e) {
      if (mounted) showSnack(context, reasonText(context, e));
    }
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final state = AppScope.of(context);
    Widget field(
      TextEditingController c,
      String label, {
      TextInputType? type,
      bool secret = false,
    }) => TextField(
      controller: c,
      keyboardType: type,
      obscureText: secret,
      autocorrect: false,
      enableSuggestions: !secret,
      decoration: InputDecoration(labelText: label),
    );
    Widget hostPort(
      TextEditingController host,
      String label,
      TextEditingController port,
    ) => Row(
      children: [
        Expanded(flex: 3, child: field(host, label, type: TextInputType.url)),
        const SizedBox(width: 8),
        Expanded(child: field(port, l.emailPort, type: TextInputType.number)),
      ],
    );
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        field(_address, l.emailAddress, type: TextInputType.emailAddress),
        hostPort(_imap, l.emailImapHost, _imapPort),
        hostPort(_smtp, l.emailSmtpHost, _smtpPort),
        field(_user, l.emailUsername),
        field(
          _password,
          _hasAccount ? l.emailPasswordKept : l.emailPassword,
          secret: true,
        ),
        const SizedBox(height: 8),
        Wrap(
          spacing: 8,
          children: [
            FilledButton(onPressed: _save, child: Text(l.emailSaveAccount)),
            if (_hasAccount)
              TextButton(
                onPressed: () async {
                  await state.node.setEmailAccount(account: null);
                  _hasAccount = false;
                  state.changed();
                },
                child: Text(l.emailRemoveAccount),
              ),
          ],
        ),
        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          title: Text(l.emailToggle),
          subtitle: Text(l.emailToggleHelp),
          value: widget.enabled,
          onChanged: _hasAccount || widget.enabled
              ? (on) async {
                  await state.node.setEmailEnabled(enabled: on);
                  state.changed();
                }
              : null,
        ),
        if (widget.enabled && _hasAccount)
          Text(
            state.node.emailOnline() ? l.emailOnline : l.emailOffline,
            style: Theme.of(context).textTheme.bodySmall,
          ),
      ],
    );
  }
}
