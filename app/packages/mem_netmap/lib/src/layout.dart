import 'dart:math' as math;
import 'dart:ui';

import 'model.dart';

/// Places nodes on rings around the centre node: the inner ring evenly, each outer node next to
/// the inner nodes it links to, so links stay short and rarely cross. Deterministic: the same
/// data gives the same picture.
class RadialLayout {
  /// Creates a layout with ring radii in logical pixels.
  const RadialLayout({
    this.innerRadius = 170,
    this.outerRadius = 330,
    this.spacing = 132,
  });

  /// Smallest radius of the inner ring.
  final double innerRadius;

  /// Smallest radius of the outer ring.
  final double outerRadius;

  /// Room each node needs along its ring; rings grow so nodes do not overlap.
  final double spacing;

  /// The radii used for [data]: at least the configured ones, larger when a ring is crowded,
  /// and the outer ring always clear of the inner one.
  (double, double) radii(NetMapData data) {
    final nInner = data.nodes.where((n) => n.ring == NetRing.inner).length;
    final nOuter = data.nodes.where((n) => n.ring == NetRing.outer).length;
    final inner = math.max(innerRadius, nInner * spacing / (2 * math.pi));
    final outer = math.max(
      math.max(outerRadius, inner + spacing * 1.2),
      nOuter * spacing / (2 * math.pi),
    );
    return (inner, outer);
  }

  /// Positions by node id, relative to the centre at (0, 0).
  Map<String, Offset> place(NetMapData data) {
    final (innerRadius, outerRadius) = radii(data);
    final out = <String, Offset>{};
    final inner = data.nodes.where((n) => n.ring == NetRing.inner).toList();
    final outer = data.nodes.where((n) => n.ring == NetRing.outer).toList();
    for (final n in data.nodes.where((n) => n.ring == NetRing.centre)) {
      out[n.id] = Offset.zero;
    }
    // Inner ring: evenly, starting at the top, clockwise.
    final innerAngle = <String, double>{};
    for (var i = 0; i < inner.length; i++) {
      final a = -math.pi / 2 + 2 * math.pi * i / inner.length;
      innerAngle[inner[i].id] = a;
      out[inner[i].id] = _polar(innerRadius, a);
    }
    if (outer.isEmpty) return out;
    // Outer ring: each node wants the mean direction of the inner nodes it links to; sort by
    // that, then spread evenly in that order so none overlap.
    final wanted = <String, double>{};
    for (final (i, n) in outer.indexed) {
      final angles = [
        for (final e in data.edges)
          if (e.to == n.id && innerAngle.containsKey(e.from))
            innerAngle[e.from]!
          else if (e.from == n.id && innerAngle.containsKey(e.to))
            innerAngle[e.to]!,
      ];
      wanted[n.id] = angles.isEmpty
          ? 2 * math.pi * i / outer.length
          : _meanAngle(angles);
    }
    final order = [...outer]
      ..sort((a, b) {
        final c = _norm(wanted[a.id]!).compareTo(_norm(wanted[b.id]!));
        return c != 0 ? c : a.id.compareTo(b.id);
      });
    final step = 2 * math.pi / order.length;
    // Rotate the evenly spread ring so it sits as close as possible to where nodes want to be.
    var shift = 0.0;
    var sx = 0.0, sy = 0.0;
    for (final (i, n) in order.indexed) {
      final d = _norm(wanted[n.id]!) - i * step;
      sx += math.cos(d);
      sy += math.sin(d);
    }
    if (sx != 0 || sy != 0) shift = math.atan2(sy, sx);
    for (final (i, n) in order.indexed) {
      out[n.id] = _polar(outerRadius, shift + i * step);
    }
    return out;
  }

  static Offset _polar(double r, double a) =>
      Offset(r * math.cos(a), r * math.sin(a));

  static double _norm(double a) {
    final t = a % (2 * math.pi);
    return t < 0 ? t + 2 * math.pi : t;
  }

  static double _meanAngle(List<double> angles) {
    var x = 0.0, y = 0.0;
    for (final a in angles) {
      x += math.cos(a);
      y += math.sin(a);
    }
    return math.atan2(y, x);
  }
}
