import 'dart:async';

import 'package:material_ui/material_ui.dart';
import 'package:mem_netmap/mem_netmap.dart';

import '../app_scope.dart';
import '../l10n/gen/app_localizations.dart';
import '../src/rust/api.dart';
import '../theme.dart';
import 'channels.dart';
import 'chat.dart';
import 'shares.dart';

/// Node ids on the map.
const _me = 'me';
const _courier = 'ch:courier';
const _contactHubs = 'devhubs';
String _channelId(Channel c) => 'ch:${c.name}';
String _hubId(String contact) => 'dh:$contact';
String _contactId(String contact) => 'c:$contact';

/// Most contacts drawn; the most recent first.
const _maxContacts = 20;

/// Lines kept in the event list.
const _maxEvents = 8;

/// This device's connections as a map: channels around it, contacts outside, live links in
/// their grade's colour, traffic as moving pulses, connects and disconnects as ripples and in a
/// short event list. Only this device's own links; tapping opens the same controls and
/// disclosures as Channels, and dropping a contact on "Contacts' hubs" asks before anything
/// changes.
class NetworkMapScreen extends StatefulWidget {
  /// Creates the screen.
  const NetworkMapScreen({super.key});

  @override
  State<NetworkMapScreen> createState() => _NetworkMapScreenState();
}

class _NetworkMapScreenState extends State<NetworkMapScreen> {
  final _controller = NetMapController();
  StreamSubscription<NodeEventItem_Traffic>? _traffic;
  NetworkSnapshot? _last;
  final _events = <String>[];
  Set<String> _edges = {};
  bool _dragging = false;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _traffic ??= AppScope.read(context).traffic.listen(_onTraffic);
  }

  @override
  void dispose() {
    _traffic?.cancel();
    _controller.dispose();
    super.dispose();
  }

  void _onTraffic(NodeEventItem_Traffic e) {
    if (!mounted) return;
    final l = AppLocalizations.of(context);
    final channelNode = _channelNode(e.channel);
    final contactNode = _contactId(e.contactId);
    final linked = _edges.contains('$channelNode>$contactNode');
    final color = _gradeColor(context, e.channel);
    if (linked) {
      e.outgoing
          ? _controller.pulse(channelNode, contactNode, color: color)
          : _controller.pulse(contactNode, channelNode, color: color);
    } else {
      e.outgoing
          ? _controller.pulse(_me, channelNode, color: color)
          : _controller.pulse(channelNode, _me, color: color);
    }
    // The list keeps what a person cares about: messages and files, not setup traffic
    // (call setup, acknowledgements, key updates), which still pulses on the map.
    if (!e.visible) return;
    final name = _name(e.contactId);
    final channel = channelLabel(l, e.channel);
    _log(
      e.outgoing
          ? l.netmapSent(name, channel)
          : l.netmapReceived(name, channel),
    );
  }

  void _log(String line) {
    final time = TimeOfDay.now().format(context);
    setState(() {
      _events.insert(0, '$time  $line');
      if (_events.length > _maxEvents) _events.removeLast();
    });
  }

  String _name(String contactId) {
    final c = AppScope.read(context).node
        .contacts()
        .where((c) => c.id == contactId)
        .firstOrNull;
    return c == null ? contactId.substring(0, 8) : contactTitle(c);
  }

  /// Connects and disconnects since the last picture, for the event list.
  void _diff(NetworkSnapshot now) {
    final before = _last;
    _last = now;
    if (before == null) return;
    final l = AppLocalizations.of(context);
    for (final c in now.channels) {
      final was = before.channels
          .where((b) => b.channel == c.channel)
          .firstOrNull;
      if (was == null || was.online == c.online || !c.enabled) continue;
      final name = channelLabel(l, c.channel);
      WidgetsBinding.instance.addPostFrameCallback(
        (_) => _log(
          c.online ? l.netmapChannelUp(name) : l.netmapChannelDown(name),
        ),
      );
    }
    String key(NetLinkItem x) => '${x.contactId}|${x.channel.name}';
    final a = {for (final x in before.links) key(x): x};
    final b = {for (final x in now.links) key(x): x};
    for (final k in b.keys.where((k) => !a.containsKey(k))) {
      final x = b[k]!;
      WidgetsBinding.instance.addPostFrameCallback(
        (_) => _log(
          l.netmapEventUp(_name(x.contactId), channelLabel(l, x.channel)),
        ),
      );
    }
    for (final k in a.keys.where((k) => !b.containsKey(k))) {
      final x = a[k]!;
      WidgetsBinding.instance.addPostFrameCallback(
        (_) => _log(
          l.netmapEventDown(_name(x.contactId), channelLabel(l, x.channel)),
        ),
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final state = AppScope.of(context);
    final snapshot = state.node.networkSnapshot();
    _diff(snapshot);
    final data = _build(context, l, state, snapshot);
    _edges = {for (final e in data.edges) e.id};
    return Scaffold(
      appBar: AppBar(
        title: Text(l.netmapTitle),
        actions: [
          IconButton(
            tooltip: l.netmapHelpTitle,
            icon: const Icon(Icons.help_outline),
            onPressed: () => _help(context),
          ),
        ],
      ),
      body: Stack(
        children: [
          Positioned.fill(
            child: NetMapView(
              data: data,
              controller: _controller,
              onTapNode: (id) => _tap(context, id, snapshot),
              onDropNode: (dragged, target) => _drop(context, dragged, target),
              onDragChanged: (id) => setState(() => _dragging = id != null),
            ),
          ),
          if (_dragging)
            Positioned(
              left: 12,
              top: 8,
              right: 12,
              child: _Hint(text: l.netmapDragHint),
            ),
          if (_events.isNotEmpty)
            Positioned(
              left: 12,
              right: 12,
              bottom: 12,
              child: _EventList(lines: _events),
            ),
        ],
      ),
    );
  }

  NetMapData _build(
    BuildContext context,
    AppLocalizations l,
    AppState state,
    NetworkSnapshot s,
  ) {
    final nodes = <NetNode>[];
    final edges = <NetEdge>[];
    final off = Theme.of(context).disabledColor;
    final clients = s.hubClients.length;
    nodes.add(
      NetNode(
        id: _me,
        ring: NetRing.centre,
        label: l.netmapThisDevice,
        subtitle: !s.serving
            ? null
            : clients > 0
            ? l.netmapServing(clients)
            : l.netmapServingIdle,
        icon: Icons.smartphone,
        state: NetNodeState.up,
        badge: s.serving && s.hubHolding > BigInt.zero
            ? '${s.hubHolding}'
            : null,
      ),
    );
    // Channels: Couriers become one "nearby devices" node; contacts' hubs get their own.
    final byChannel = {for (final c in s.channels) c.channel: c};
    for (final c in s.channels) {
      if (c.channel == Channel.deviceHub) continue;
      if (c.channel == Channel.email && !c.enabled) continue;
      final courier = c.channel == Channel.courier;
      final id = courier ? _courier : _channelId(c.channel);
      final nearby = s.channels.fold<int>(0, (n, x) => n + x.peers);
      final color = MemColors.grade(context, c.grade);
      final nodeState = !c.enabled
          ? NetNodeState.off
          : courier
          ? (nearby > 0 ? NetNodeState.up : NetNodeState.idle)
          : c.online
          ? NetNodeState.up
          : c.channel == Channel.hub
          ? NetNodeState.connecting
          : NetNodeState.idle;
      nodes.add(
        NetNode(
          id: id,
          ring: NetRing.inner,
          label: courier ? l.netmapNearby : channelLabel(l, c.channel),
          subtitle: courier ? l.netmapNearbyCount(nearby) : c.grade,
          icon: channelIcon(c.channel),
          state: nodeState,
          accent: color,
        ),
      );
      edges.add(
        NetEdge(from: _me, to: id, style: _spoke(nodeState, color, off)),
      );
    }
    // Contacts' devices used as hubs, and the drop target to add one.
    final p3 = MemColors.grade(context, 'P3');
    final names = {
      for (final c in state.node.contacts()) c.id: contactTitle(c),
    };
    for (final h in s.deviceHubs) {
      final id = _hubId(h.contactId);
      final st = h.online ? NetNodeState.up : NetNodeState.idle;
      nodes.add(
        NetNode(
          id: id,
          ring: NetRing.inner,
          label: l.netmapDevice(
            names[h.contactId] ?? h.contactId.substring(0, 8),
          ),
          subtitle: 'P3',
          icon: Icons.tablet_android_outlined,
          state: st,
          accent: p3,
        ),
      );
      edges.add(NetEdge(from: _me, to: id, style: _spoke(st, p3, off)));
    }
    final hubChannel = byChannel[Channel.deviceHub];
    if (hubChannel != null && hubChannel.enabled) {
      nodes.add(
        NetNode(
          id: _contactHubs,
          ring: NetRing.inner,
          label: l.netmapContactHubs,
          subtitle: l.netmapDropHere,
          icon: Icons.hub_outlined,
          state: NetNodeState.idle,
          accent: p3,
          acceptsDrop: true,
        ),
      );
      edges.add(
        NetEdge(
          from: _me,
          to: _contactHubs,
          style: NetEdgeStyle(color: p3, dashed: true, glow: false, width: 1.5),
        ),
      );
    }
    // Contacts, the most recent first; live links in their grade's colour, otherwise one
    // dashed link to what will carry to them later.
    final usedAsHub = {for (final h in s.deviceHubs) h.contactId};
    final contacts = state.node
        .contacts()
        .where((c) => !c.requested)
        .take(_maxContacts);
    final hubOnline = byChannel[Channel.hub]?.online ?? false;
    for (final c in contacts) {
      final id = _contactId(c.id);
      final live = s.links.where((x) => x.contactId == c.id).toList();
      final shared = state.node
          .sharedWithMe(contactId: c.id)
          .fold<int>(0, (n, x) => n + x.files.length);
      nodes.add(
        NetNode(
          id: id,
          ring: NetRing.outer,
          label: contactTitle(c),
          subtitle: s.hubClients.contains(c.id) ? l.netmapOnYourHub : null,
          icon: Icons.person_outline,
          state: live.isEmpty ? NetNodeState.idle : NetNodeState.up,
          accent: live.isEmpty
              ? null
              : MemColors.grade(context, _gradeOf(s, live.first.channel)),
          badge: shared > 0 ? '📁$shared' : null,
          draggable: !usedAsHub.contains(c.id),
        ),
      );
      for (final x in live) {
        final grade = _gradeOf(s, x.channel);
        edges.add(
          NetEdge(
            from: _channelId(x.channel),
            to: id,
            style: NetEdgeStyle(
              color: MemColors.grade(context, grade),
              label: grade,
            ),
          ),
        );
      }
      if (live.isEmpty && hubOnline) {
        edges.add(
          NetEdge(
            from: _channelId(Channel.hub),
            to: id,
            style: NetEdgeStyle(
              color: MemColors.grade(context, 'P2'),
              dashed: true,
              glow: false,
              width: 1.5,
            ),
          ),
        );
      }
    }
    return NetMapData(nodes: nodes, edges: edges);
  }

  static String _gradeOf(NetworkSnapshot s, Channel c) =>
      s.channels.where((x) => x.channel == c).firstOrNull?.grade ?? 'P1';

  static NetEdgeStyle _spoke(NetNodeState st, Color color, Color off) =>
      switch (st) {
        NetNodeState.up => NetEdgeStyle(color: color),
        NetNodeState.off => NetEdgeStyle(color: off, glow: false, width: 1.2),
        _ => NetEdgeStyle(
          color: color.withValues(alpha: 0.6),
          dashed: true,
          glow: false,
          width: 1.5,
        ),
      };

  String _channelNode(Channel c) => switch (c) {
    Channel.courier => _courier,
    Channel.deviceHub => _contactHubs,
    _ => _channelId(c),
  };

  Color _gradeColor(BuildContext context, Channel c) {
    final s = _last;
    return MemColors.grade(context, s == null ? 'P1' : _gradeOf(s, c));
  }

  Future<void> _tap(BuildContext context, String id, NetworkSnapshot s) async {
    final state = AppScope.read(context);
    if (id.startsWith('c:')) {
      await _contactSheet(context, id.substring(2));
      return;
    }
    final channel = switch (id) {
      _courier => Channel.courier,
      _contactHubs || _me => Channel.deviceHub,
      _ when id.startsWith('dh:') => null,
      _ => Channel.values.where((c) => _channelId(c) == id).firstOrNull,
    };
    if (id.startsWith('dh:')) {
      await _contactSheet(context, id.substring(3));
      return;
    }
    if (channel == null) return;
    final info = state.node
        .channels(locale: localeCode(context))
        .where((c) => c.channel == channel)
        .firstOrNull;
    if (info == null || !context.mounted) return;
    await showModalBottomSheet<void>(
      context: context,
      isScrollControlled: true,
      showDragHandle: true,
      builder: (context) => SafeArea(
        child: SingleChildScrollView(
          padding: const EdgeInsets.fromLTRB(16, 0, 16, 16),
          child: ChannelCard(info: info),
        ),
      ),
    );
  }

  Future<void> _contactSheet(BuildContext context, String contactId) async {
    final l = AppLocalizations.of(context);
    final state = AppScope.read(context);
    final c = state.node.contacts().where((c) => c.id == contactId).firstOrNull;
    if (c == null) return;
    await showModalBottomSheet<void>(
      context: context,
      showDragHandle: true,
      // As tall as its content (up to the screen), scrolling only when it does not fit.
      isScrollControlled: true,
      builder: (context) => SafeArea(
        child: StatefulBuilder(
          builder: (context, setSheet) {
            final now =
                state.node
                    .contacts()
                    .where((x) => x.id == contactId)
                    .firstOrNull ??
                c;
            return ListView(
              shrinkWrap: true,
              padding: const EdgeInsets.only(bottom: 8),
              children: [
                ListTile(
                  title: Text(
                    contactTitle(now),
                    style: Theme.of(context).textTheme.titleMedium,
                  ),
                ),
                ListTile(
                  leading: const Icon(Icons.chat_bubble_outline),
                  title: Text(l.netmapOpenChat),
                  onTap: () {
                    Navigator.pop(context);
                    Navigator.of(context).push(
                      MaterialPageRoute<void>(
                        builder: (_) => ChatScreen(contactId: contactId),
                      ),
                    );
                  },
                ),
                ListTile(
                  leading: const Icon(Icons.folder_outlined),
                  title: Text(l.sharedWithMe),
                  onTap: () {
                    Navigator.pop(context);
                    Navigator.of(context).push(
                      MaterialPageRoute<void>(
                        builder: (_) =>
                            SharedWithMeScreen(contactId: contactId),
                      ),
                    );
                  },
                ),
                ListTile(
                  leading: const Icon(Icons.folder_shared_outlined),
                  title: Text(l.sharesMine(contactTitle(now))),
                  onTap: () {
                    Navigator.pop(context);
                    Navigator.of(context).push(
                      MaterialPageRoute<void>(
                        builder: (_) => SharesScreen(contactId: contactId),
                      ),
                    );
                  },
                ),
                SwitchListTile(
                  secondary: const Icon(Icons.tablet_android_outlined),
                  title: Text(l.deviceHubUse),
                  subtitle: Text(l.deviceHubUseHelp(contactTitle(now))),
                  value: now.useAsHub,
                  onChanged: (on) async {
                    await state.node.setUseDeviceHub(
                      contactId: contactId,
                      enabled: on,
                    );
                    state.changed();
                    setSheet(() {});
                  },
                ),
                const Divider(),
                ListTile(
                  leading: Icon(
                    Icons.person_remove_outlined,
                    color: Theme.of(context).colorScheme.error,
                  ),
                  title: Text(
                    l.removeContact,
                    style: TextStyle(
                      color: Theme.of(context).colorScheme.error,
                    ),
                  ),
                  onTap: () async {
                    Navigator.pop(context);
                    await _remove(this.context, contactId);
                  },
                ),
              ],
            );
          },
        ),
      ),
    );
  }

  /// Removes a contact after the same confirmation as the contact screen.
  Future<void> _remove(BuildContext context, String contactId) async {
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
    try {
      await state.node.removeContact(contactId: contactId);
      state.changed();
    } on Object catch (e) {
      if (context.mounted) showSnack(context, reasonText(context, e));
    }
  }

  /// What the map shows and how to use it, with its own scrolling.
  Future<void> _help(BuildContext context) {
    final l = AppLocalizations.of(context);
    return showModalBottomSheet<void>(
      context: context,
      isScrollControlled: true,
      showDragHandle: true,
      builder: (context) => DraggableScrollableSheet(
        expand: false,
        initialChildSize: 0.6,
        maxChildSize: 0.92,
        builder: (context, scroll) => _HelpBody(l: l, scroll: scroll),
      ),
    );
  }

  /// A contact dropped on "Contacts' hubs": the same words as the contact screen, then the
  /// user decides.
  Future<void> _drop(
    BuildContext context,
    String dragged,
    String target,
  ) async {
    if (target != _contactHubs || !dragged.startsWith('c:')) return;
    final l = AppLocalizations.of(context);
    final state = AppScope.read(context);
    final contactId = dragged.substring(2);
    final name = _name(contactId);
    final ok = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        icon: const Icon(Icons.tablet_android_outlined),
        title: Text(l.netmapUseHubTitle(name)),
        content: Text(l.deviceHubUseHelp(name)),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context, false),
            child: Text(l.cancel),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(context, true),
            child: Text(l.netmapUse),
          ),
        ],
      ),
    );
    if (ok != true || !context.mounted) return;
    try {
      await state.node.setUseDeviceHub(contactId: contactId, enabled: true);
      state.changed();
    } on Object catch (e) {
      if (context.mounted) showSnack(context, reasonText(context, e));
    }
  }
}

/// The help text: what the map is, the lines, taps, dragging and zoom.
class _HelpBody extends StatelessWidget {
  const _HelpBody({required this.l, required this.scroll});

  final AppLocalizations l;
  final ScrollController scroll;

  @override
  Widget build(BuildContext context) {
    final text = Theme.of(context).textTheme;
    final color = Theme.of(context).colorScheme.onSurfaceVariant;
    Widget heading(String s) => Padding(
      padding: const EdgeInsets.only(top: 16, bottom: 6),
      child: Text(s, style: text.titleSmall),
    );
    Widget line(String label, {bool dashed = false, double opacity = 1}) =>
        Padding(
          padding: const EdgeInsets.symmetric(vertical: 4),
          child: Row(
            children: [
              SizedBox(
                width: 36,
                height: 10,
                child: CustomPaint(
                  painter: _LegendLine(
                    color.withValues(alpha: opacity),
                    dashed,
                  ),
                ),
              ),
              const SizedBox(width: 12),
              Expanded(child: Text(label, style: text.bodyMedium)),
            ],
          ),
        );
    return Scrollbar(
      controller: scroll,
      thumbVisibility: true,
      child: ListView(
        controller: scroll,
        padding: const EdgeInsets.fromLTRB(20, 0, 20, 24),
        children: [
          Text(l.netmapHelpTitle, style: text.titleLarge),
          const SizedBox(height: 8),
          Text(l.netmapHelpWhat, style: text.bodyMedium),
          heading(l.netmapHelpLines),
          line(l.netmapLegendLive),
          line(l.netmapLegendLater, dashed: true),
          line(l.netmapLegendOff, opacity: 0.35),
          const SizedBox(height: 6),
          Text(l.netmapHelpColours, style: text.bodyMedium),
          const SizedBox(height: 6),
          Text(l.netmapHelpEvents, style: text.bodyMedium),
          heading(l.netmapHelpTapTitle),
          Text(l.netmapHelpTap, style: text.bodyMedium),
          heading(l.netmapHelpDragTitle),
          Text(l.netmapHelpDrag, style: text.bodyMedium),
          const SizedBox(height: 12),
          Text(l.netmapHelpZoom, style: text.bodySmall),
        ],
      ),
    );
  }
}

/// A line of guidance over the map while dragging.
class _Hint extends StatelessWidget {
  const _Hint({required this.text});

  final String text;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Material(
      color: scheme.inverseSurface,
      borderRadius: BorderRadius.circular(12),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
        child: Row(
          children: [
            Icon(
              Icons.pan_tool_alt_outlined,
              size: 18,
              color: scheme.onInverseSurface,
            ),
            const SizedBox(width: 10),
            Expanded(
              child: Text(
                text,
                style: TextStyle(color: scheme.onInverseSurface),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _LegendLine extends CustomPainter {
  _LegendLine(this.color, this.dashed);

  final Color color;
  final bool dashed;

  @override
  void paint(Canvas canvas, Size size) {
    final p = Paint()
      ..color = color
      ..strokeWidth = 2
      ..strokeCap = StrokeCap.round;
    final y = size.height / 2;
    if (!dashed) {
      canvas.drawLine(Offset(0, y), Offset(size.width, y), p);
      return;
    }
    for (var x = 0.0; x < size.width; x += 7) {
      canvas.drawLine(Offset(x, y), Offset(x + 4, y), p);
    }
  }

  @override
  bool shouldRepaint(covariant _LegendLine old) =>
      old.color != color || old.dashed != dashed;
}

/// The latest connects, disconnects and traffic, newest first.
class _EventList extends StatelessWidget {
  const _EventList({required this.lines});

  final List<String> lines;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Material(
      color: scheme.surfaceContainerHigh.withValues(alpha: 0.92),
      borderRadius: BorderRadius.circular(12),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          mainAxisSize: MainAxisSize.min,
          children: [
            for (final (i, line) in lines.take(4).indexed)
              Opacity(
                opacity: 1 - i * 0.2,
                child: Text(
                  line,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: Theme.of(context).textTheme.bodySmall,
                ),
              ),
          ],
        ),
      ),
    );
  }
}
