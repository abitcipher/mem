// Node cards follow graph_flow (MIT, Copyright (c) 2025 Benedikt Kotas): rounded dark or light
// cards with a thin border and a soft shadow on a grid, and a live beam while connecting.
// See THIRD_PARTY_NOTICES.md.

import 'dart:math' as math;

import 'package:material_ui/material_ui.dart';
import 'package:flutter/scheduler.dart';

import 'layout.dart';
import 'model.dart';
import 'painter.dart';
import 'theme.dart';

/// Sends one-off events to a [NetMapView]: traffic along a link.
class NetMapController extends ChangeNotifier {
  final _pending = <(String, String, Color?)>[];

  /// Something went from node [from] to node [to] (a message, an ack, a file).
  void pulse(String from, String to, {Color? color}) {
    _pending.add((from, to, color));
    notifyListeners();
  }

  List<(String, String, Color?)> _take() {
    final out = [..._pending];
    _pending.clear();
    return out;
  }
}

/// The map: nodes on rings, links, pulses; tap a node, or long-press a draggable node and drop
/// it on another to ask for a connection.
class NetMapView extends StatefulWidget {
  /// Creates the view.
  const NetMapView({
    required this.data,
    this.controller,
    this.layout = const RadialLayout(),
    this.onTapNode,
    this.onDropNode,
    this.onDragChanged,
    super.key,
  });

  /// What to show.
  final NetMapData data;

  /// Traffic events.
  final NetMapController? controller;

  /// Where nodes go.
  final RadialLayout layout;

  /// A node was tapped.
  final void Function(String id)? onTapNode;

  /// Node [dragged] was dropped on node [target] (which accepts drops).
  final void Function(String dragged, String target)? onDropNode;

  /// A drag started (the node's id) or ended (`null`), e.g. to show a hint.
  final void Function(String? dragged)? onDragChanged;

  @override
  State<NetMapView> createState() => _NetMapViewState();
}

/// Card sizes.
const _cardWidth = 104.0;
const _cardHeight = 96.0;
const _centreSize = 120.0;
const _margin = 120.0;

class _NetMapViewState extends State<NetMapView>
    with SingleTickerProviderStateMixin {
  late final Ticker _ticker;
  final _clock = ValueNotifier<double>(0);
  final _transform = TransformationController();
  var _pulses = <NetPulse>[];
  final _ripples = <String, double>{};
  String? _dragFrom;
  Offset? _dragTo;
  String? _hover;
  bool _placed = false;

  double get _side => 2 * (widget.layout.radii(widget.data).$2 + _margin);
  Offset get _centre => Offset(_side / 2, _side / 2);

  @override
  void initState() {
    super.initState();
    _ticker = createTicker((elapsed) {
      _clock.value = elapsed.inMicroseconds / 1e6;
      _pulses = [
        for (final p in _pulses)
          if (_clock.value - p.startSeconds < NetPulse.seconds) p,
      ];
      _ripples.removeWhere((_, start) => _clock.value - start > 1.2);
      if (!_animating) _ticker.stop();
    });
    widget.controller?.addListener(_onEvents);
    _wake();
  }

  @override
  void didUpdateWidget(NetMapView old) {
    super.didUpdateWidget(old);
    if (old.controller != widget.controller) {
      old.controller?.removeListener(_onEvents);
      widget.controller?.addListener(_onEvents);
    }
    // A ripple where a node connected or disconnected.
    final before = {for (final n in old.data.nodes) n.id: n.state};
    for (final n in widget.data.nodes) {
      final was = before[n.id];
      if (was != null && was != n.state) _ripples[n.id] = _clock.value;
    }
    _wake();
  }

  /// Something moves: dots, pulses, ripples, a pulsing ring or a drag.
  bool get _animating =>
      _pulses.isNotEmpty ||
      _ripples.isNotEmpty ||
      _dragFrom != null ||
      widget.data.edges.any((e) => e.style.flow != NetFlow.none) ||
      widget.data.nodes.any((n) => n.state == NetNodeState.connecting);

  void _wake() {
    if (_animating && !_ticker.isActive) _ticker.start();
  }

  void _onEvents() {
    final now = _clock.value;
    setState(() {
      _pulses = [
        ..._pulses,
        for (final (from, to, color) in widget.controller!._take())
          NetPulse(from: from, to: to, startSeconds: now, color: color),
      ];
    });
    _wake();
  }

  @override
  void dispose() {
    widget.controller?.removeListener(_onEvents);
    _ticker.dispose();
    _clock.dispose();
    _transform.dispose();
    super.dispose();
  }

  /// Fits the nodes (not the whole canvas) into the view once, centred.
  void _fit(Size viewport, double extent) {
    if (_placed || viewport.isEmpty) return;
    _placed = true;
    final scale = (math.min(viewport.width, viewport.height) / (2 * extent))
        .clamp(0.3, 1.6);
    final dx = viewport.width / 2 - _side / 2 * scale;
    final dy = viewport.height / 2 - _side / 2 * scale;
    _transform.value = Matrix4.identity()
      ..translateByDouble(dx, dy, 0, 1)
      ..scaleByDouble(scale, scale, 1, 1);
  }

  String? _dropTargetAt(Offset at, Map<String, Offset> positions) {
    String? best;
    var bestDistance = 64.0;
    for (final n in widget.data.nodes) {
      if (!n.acceptsDrop || n.id == _dragFrom) continue;
      final p = positions[n.id];
      if (p == null) continue;
      final d = (p - at).distance;
      if (d < bestDistance) {
        bestDistance = d;
        best = n.id;
      }
    }
    return best;
  }

  @override
  Widget build(BuildContext context) {
    final theme = NetMapTheme.of(context);
    final placed = widget.layout.place(widget.data);
    final positions = {
      for (final e in placed.entries) e.key: e.value + _centre,
    };
    // How far nodes reach from the centre, cards included.
    final extent =
        placed.values.fold<double>(0, (m, p) => math.max(m, p.distance)) +
        _cardWidth * 0.75;
    return LayoutBuilder(
      builder: (context, box) {
        _fit(box.biggest, extent);
        return ColoredBox(
          color: theme.background,
          child: InteractiveViewer(
            transformationController: _transform,
            constrained: false,
            boundaryMargin: const EdgeInsets.all(200),
            minScale: 0.3,
            maxScale: 3,
            child: SizedBox(
              width: _side,
              height: _side,
              child: Stack(
                children: [
                  Positioned.fill(
                    child: CustomPaint(
                      painter: NetLinkPainter(
                        data: widget.data,
                        positions: positions,
                        theme: theme,
                        clock: _clock,
                        pulses: _pulses,
                        dragFrom: _dragFrom,
                        dragTo: _dragTo,
                        highlight: _hover ?? _dragFrom,
                      ),
                    ),
                  ),
                  Positioned.fill(
                    child: IgnorePointer(
                      child: CustomPaint(
                        painter: _RipplePainter(
                          ripples: Map.of(_ripples),
                          positions: positions,
                          clock: _clock,
                          color: theme.accent,
                        ),
                      ),
                    ),
                  ),
                  for (final n in widget.data.nodes)
                    if (positions[n.id] case final p?)
                      _positioned(n, p, theme, positions),
                ],
              ),
            ),
          ),
        );
      },
    );
  }

  Widget _positioned(
    NetNode n,
    Offset p,
    NetMapTheme theme,
    Map<String, Offset> positions,
  ) {
    final centre = n.ring == NetRing.centre;
    final w = centre ? _centreSize : _cardWidth;
    final h = centre ? _centreSize : _cardHeight;
    final topLeft = p - Offset(w / 2, h / 2);
    Widget card = _NodeCard(
      node: n,
      theme: theme,
      clock: _clock,
      centre: centre,
      highlighted: _hover == n.id,
      // While dragging, every place the node may be dropped on lights up.
      candidate: _dragFrom != null && n.acceptsDrop && n.id != _dragFrom,
    );
    card = GestureDetector(
      onTap: widget.onTapNode == null ? null : () => widget.onTapNode!(n.id),
      onLongPressStart: !n.draggable
          ? null
          : (d) {
              setState(() {
                _dragFrom = n.id;
                _dragTo = topLeft + d.localPosition;
              });
              widget.onDragChanged?.call(n.id);
              _wake();
            },
      onLongPressMoveUpdate: !n.draggable
          ? null
          : (d) => setState(() {
              _dragTo = topLeft + d.localPosition;
              _hover = _dropTargetAt(_dragTo!, positions);
            }),
      onLongPressEnd: !n.draggable
          ? null
          : (d) {
              final target = _dropTargetAt(
                topLeft + d.localPosition,
                positions,
              );
              final from = _dragFrom;
              setState(() {
                _dragFrom = null;
                _dragTo = null;
                _hover = null;
              });
              widget.onDragChanged?.call(null);
              if (target != null && from != null) {
                widget.onDropNode?.call(from, target);
              }
            },
      child: card,
    );
    return Positioned(
      left: topLeft.dx,
      top: topLeft.dy,
      width: w,
      height: h,
      child: card,
    );
  }
}

class _RipplePainter extends CustomPainter {
  _RipplePainter({
    required this.ripples,
    required this.positions,
    required this.clock,
    required this.color,
  }) : super(repaint: clock);

  final Map<String, double> ripples;
  final Map<String, Offset> positions;
  final ValueNotifier<double> clock;
  final Color color;

  @override
  void paint(Canvas canvas, Size size) {
    for (final MapEntry(key: id, value: start) in ripples.entries) {
      final p = positions[id];
      if (p == null) continue;
      final t = ((clock.value - start) / 1.2).clamp(0.0, 1.0);
      canvas.drawCircle(
        p,
        40 + 60 * t,
        Paint()
          ..color = color.withValues(alpha: 0.5 * (1 - t))
          ..style = PaintingStyle.stroke
          ..strokeWidth = 3 * (1 - t) + 0.5,
      );
    }
  }

  @override
  bool shouldRepaint(covariant _RipplePainter old) => true;
}

/// A node: a card with an icon in a state ring, a label and a second line.
class _NodeCard extends StatelessWidget {
  const _NodeCard({
    required this.node,
    required this.theme,
    required this.clock,
    required this.centre,
    required this.highlighted,
    this.candidate = false,
  });

  final NetNode node;
  final bool candidate;
  final NetMapTheme theme;
  final ValueNotifier<double> clock;
  final bool centre;
  final bool highlighted;

  @override
  Widget build(BuildContext context) {
    final off = node.state == NetNodeState.off;
    final ring = off ? theme.offColor : node.accent ?? theme.accent;
    final iconSize = centre ? 30.0 : 22.0;
    final card = Container(
      decoration: BoxDecoration(
        color: theme.nodeFill,
        borderRadius: BorderRadius.circular(centre ? 60 : 14),
        border: Border.all(
          color: highlighted
              ? ring
              : candidate
              ? ring.withValues(alpha: 0.7)
              : theme.nodeBorder,
          width: highlighted || candidate ? 2.5 : 1.5,
        ),
        boxShadow: [
          BoxShadow(
            color: theme.nodeShadow,
            blurRadius: 10,
            offset: const Offset(0, 4),
          ),
          if (highlighted || candidate)
            BoxShadow(
              color: ring.withValues(alpha: highlighted ? 0.45 : 0.25),
              blurRadius: 18,
              spreadRadius: 2,
            ),
        ],
      ),
      padding: const EdgeInsets.all(6),
      // Large text sizes shrink the content rather than overflow the card.
      child: FittedBox(
        fit: BoxFit.scaleDown,
        child: SizedBox(
          width: (centre ? _centreSize : _cardWidth) - 12,
          child: Column(
            mainAxisSize: MainAxisSize.min,
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              SizedBox(
                width: iconSize + 18,
                height: iconSize + 18,
                child: CustomPaint(
                  painter: _StateRing(
                    state: node.state,
                    color: ring,
                    clock: clock,
                  ),
                  child: Center(
                    child: Icon(
                      node.icon,
                      size: iconSize,
                      color: off ? theme.offColor : theme.text,
                    ),
                  ),
                ),
              ),
              const SizedBox(height: 4),
              Text(
                node.label,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                textAlign: TextAlign.center,
                style: TextStyle(
                  color: off ? theme.subtleText : theme.text,
                  fontSize: 12,
                  fontWeight: FontWeight.w600,
                ),
              ),
              if (node.subtitle case final s?)
                Text(
                  s,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  textAlign: TextAlign.center,
                  style: TextStyle(color: theme.subtleText, fontSize: 10),
                ),
            ],
          ),
        ),
      ),
    );
    return Semantics(
      button: true,
      label: [node.label, ?node.subtitle].join(', '),
      child: Stack(
        clipBehavior: Clip.none,
        children: [
          Positioned.fill(child: card),
          if (node.badge case final b?)
            Positioned(
              right: -6,
              top: -6,
              child: Container(
                padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
                decoration: BoxDecoration(
                  color: ring,
                  borderRadius: BorderRadius.circular(10),
                ),
                child: Text(
                  b,
                  style: TextStyle(
                    color: theme.nodeFill,
                    fontSize: 10,
                    fontWeight: FontWeight.bold,
                  ),
                ),
              ),
            ),
        ],
      ),
    );
  }
}

/// The ring around a node's icon: full when up, thin when idle, dotted grey when off, a
/// sweeping arc while connecting.
class _StateRing extends CustomPainter {
  _StateRing({required this.state, required this.color, required this.clock})
    : super(repaint: state == NetNodeState.connecting ? clock : null);

  final NetNodeState state;
  final Color color;
  final ValueNotifier<double> clock;

  @override
  void paint(Canvas canvas, Size size) {
    final c = size.center(Offset.zero);
    final r = size.shortestSide / 2 - 2;
    final paint = Paint()
      ..style = PaintingStyle.stroke
      ..color = color;
    switch (state) {
      case NetNodeState.up:
        canvas.drawCircle(
          c,
          r + 1,
          Paint()..color = color.withValues(alpha: 0.18),
        );
        canvas.drawCircle(c, r, paint..strokeWidth = 2.5);
      case NetNodeState.idle:
        canvas.drawCircle(c, r, paint..strokeWidth = 1.2);
      case NetNodeState.off:
        final dots = Paint()..color = color.withValues(alpha: 0.6);
        for (var i = 0; i < 12; i++) {
          final a = 2 * math.pi * i / 12;
          canvas.drawCircle(
            c + Offset(math.cos(a), math.sin(a)) * r,
            1.2,
            dots,
          );
        }
      case NetNodeState.connecting:
        final start = (clock.value * 2 * math.pi) % (2 * math.pi);
        canvas.drawCircle(
          c,
          r,
          Paint()
            ..style = PaintingStyle.stroke
            ..strokeWidth = 1
            ..color = color.withValues(alpha: 0.3),
        );
        canvas.drawArc(
          Rect.fromCircle(center: c, radius: r),
          start,
          math.pi * 0.6,
          false,
          paint
            ..strokeWidth = 2.5
            ..strokeCap = StrokeCap.round,
        );
    }
  }

  @override
  bool shouldRepaint(covariant _StateRing old) =>
      old.state != state || old.color != color;
}
