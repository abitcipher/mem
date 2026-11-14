import 'package:flutter/widgets.dart';

/// Where a node sits on the map.
enum NetRing {
  /// The device itself, in the middle.
  centre,

  /// Around it: the ways it connects (channels, hubs).
  inner,

  /// Outside: who it reaches.
  outer,
}

/// A node's connection state, shown by its ring.
enum NetNodeState {
  /// Turned off: drawn faded.
  off,

  /// On, nothing connected.
  idle,

  /// Trying to connect: a pulsing ring.
  connecting,

  /// Connected.
  up,
}

/// One node on the map.
@immutable
class NetNode {
  /// Creates a node.
  const NetNode({
    required this.id,
    required this.ring,
    required this.label,
    required this.icon,
    this.subtitle,
    this.state = NetNodeState.idle,
    this.accent,
    this.badge,
    this.draggable = false,
    this.acceptsDrop = false,
  });

  /// Unique id.
  final String id;

  /// Which ring it sits on.
  final NetRing ring;

  /// Name shown under the icon.
  final String label;

  /// Second line, e.g. a state or a count.
  final String? subtitle;

  /// Icon in the node.
  final IconData icon;

  /// Its state.
  final NetNodeState state;

  /// Colour of its ring; the theme's accent when null.
  final Color? accent;

  /// A small badge on the corner (a count, a symbol).
  final String? badge;

  /// The user may drag it onto another node.
  final bool draggable;

  /// Another node may be dropped on it.
  final bool acceptsDrop;
}

/// Direction of the moving dots on a link.
enum NetFlow {
  /// No dots.
  none,

  /// From `from` to `to`.
  forward,

  /// From `to` to `from`.
  backward,
}

/// How one link is drawn.
@immutable
class NetEdgeStyle {
  /// Creates a style.
  const NetEdgeStyle({
    required this.color,
    this.width = 2.5,
    this.dashed = false,
    this.glow = true,
    this.flow = NetFlow.none,
    this.label,
  });

  /// Line colour (its glow uses the same hue).
  final Color color;

  /// Line width.
  final double width;

  /// Dashed: reachable later, not connected now.
  final bool dashed;

  /// Soft glow around the line.
  final bool glow;

  /// Steady moving dots.
  final NetFlow flow;

  /// Short text at the middle of the link, e.g. a grade.
  final String? label;

  @override
  bool operator ==(Object other) =>
      other is NetEdgeStyle &&
      other.color == color &&
      other.width == width &&
      other.dashed == dashed &&
      other.glow == glow &&
      other.flow == flow &&
      other.label == label;

  @override
  int get hashCode => Object.hash(color, width, dashed, glow, flow, label);
}

/// A link between two nodes.
@immutable
class NetEdge {
  /// Creates a link.
  const NetEdge({required this.from, required this.to, required this.style});

  /// Node id at one end (usually nearer the centre).
  final String from;

  /// Node id at the other end.
  final String to;

  /// How it is drawn.
  final NetEdgeStyle style;

  /// A stable id for the link.
  String get id => '$from>$to';
}

/// Everything on the map at one moment.
@immutable
class NetMapData {
  /// Creates the map's contents.
  const NetMapData({required this.nodes, required this.edges});

  /// Nodes; exactly one should be on [NetRing.centre].
  final List<NetNode> nodes;

  /// Links between them.
  final List<NetEdge> edges;
}
