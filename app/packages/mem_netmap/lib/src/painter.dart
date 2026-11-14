// Adapted from graph_flow (MIT, Copyright (c) 2025 Benedikt Kotas): the connection painting of
// its DataConnectionPainter (layered glow strokes instead of blur, dots moving along the path
// with layered halos, the live preview beam), changed to style every link on its own, draw
// dashes and labels, and run one-shot traffic pulses. See THIRD_PARTY_NOTICES.md.

import 'dart:math' as math;

import 'package:material_ui/material_ui.dart';

import 'model.dart';
import 'theme.dart';

/// One message, ack or file passing along a link, drawn once from end to end.
@immutable
class NetPulse {
  /// Creates a pulse that started at [startSeconds] on the map's clock.
  const NetPulse({
    required this.from,
    required this.to,
    required this.startSeconds,
    this.color,
  });

  /// Node it leaves.
  final String from;

  /// Node it reaches.
  final String to;

  /// When it started, seconds on the map's clock.
  final double startSeconds;

  /// Its colour; the link's when null.
  final Color? color;

  /// How long a pulse takes to cross a link.
  static const seconds = 0.9;
}

/// Paints the grid, the links with their glow, moving dots, pulses and the drag beam.
class NetLinkPainter extends CustomPainter {
  /// Creates the painter. [positions] are in canvas coordinates.
  NetLinkPainter({
    required this.data,
    required this.positions,
    required this.theme,
    required this.clock,
    required this.pulses,
    this.dragFrom,
    this.dragTo,
    this.highlight,
  }) : super(repaint: clock);

  /// What to draw.
  final NetMapData data;

  /// Node centres.
  final Map<String, Offset> positions;

  /// Colours and effects.
  final NetMapTheme theme;

  /// Seconds since the map started; repaints on every change.
  final ValueNotifier<double> clock;

  /// Pulses still running.
  final List<NetPulse> pulses;

  /// A drag in progress: from this node to [dragTo].
  final String? dragFrom;

  /// Where the finger is.
  final Offset? dragTo;

  /// Node whose links are drawn stronger (the one being dragged over or pressed).
  final String? highlight;

  @override
  void paint(Canvas canvas, Size size) {
    _grid(canvas, size);
    final t = clock.value;
    for (final e in data.edges) {
      final a = positions[e.from];
      final b = positions[e.to];
      if (a == null || b == null) continue;
      final strong =
          highlight != null && (e.from == highlight || e.to == highlight);
      final path = _curve(a, b);
      _line(canvas, path, e.style, strong);
      if (e.style.flow != NetFlow.none) {
        _dots(
          canvas,
          path,
          e.style,
          t,
          backward: e.style.flow == NetFlow.backward,
        );
      }
      if (e.style.label case final label?) {
        _label(canvas, path, label, e.style.color);
      }
    }
    for (final p in pulses) {
      final progress = (t - p.startSeconds) / NetPulse.seconds;
      if (progress < 0 || progress > 1) continue;
      final edge = data.edges.where(
        (e) =>
            (e.from == p.from && e.to == p.to) ||
            (e.from == p.to && e.to == p.from),
      );
      if (edge.isEmpty) continue;
      final e = edge.first;
      final a = positions[e.from];
      final b = positions[e.to];
      if (a == null || b == null) continue;
      final forward = e.from == p.from;
      _pulse(
        canvas,
        _curve(a, b),
        p.color ?? e.style.color,
        forward ? progress : 1 - progress,
      );
    }
    if (dragFrom != null && dragTo != null) {
      final a = positions[dragFrom];
      if (a != null) {
        final beam = NetEdgeStyle(color: theme.accent, width: 3);
        final path = _curve(a, dragTo!);
        _line(canvas, path, beam, true);
        _dots(canvas, path, beam, t, backward: false);
      }
    }
  }

  void _grid(Canvas canvas, Size size) {
    final paint = Paint()
      ..color = theme.gridLine
      ..strokeWidth = 1;
    final s = theme.gridSpacing;
    for (var x = 0.0; x <= size.width; x += s) {
      canvas.drawLine(Offset(x, 0), Offset(x, size.height), paint);
    }
    for (var y = 0.0; y <= size.height; y += s) {
      canvas.drawLine(Offset(0, y), Offset(size.width, y), paint);
    }
  }

  /// A gentle curve: a quadratic Bezier bent sideways by a tenth of the length, so links
  /// read as organic rather than as spokes.
  static Path _curve(Offset a, Offset b) {
    // Glow strokes vanish on a zero-size bounding box; nudge degenerate links.
    final end = (b - a).distance < 0.01 ? b + const Offset(0.01, 0.01) : b;
    final mid = (a + end) / 2;
    final d = end - a;
    final normal = Offset(-d.dy, d.dx) / math.max(d.distance, 1);
    final control = mid + normal * (d.distance * 0.1);
    return Path()
      ..moveTo(a.dx, a.dy)
      ..quadraticBezierTo(control.dx, control.dy, end.dx, end.dy);
  }

  void _line(Canvas canvas, Path path, NetEdgeStyle style, bool strong) {
    final drawn = style.dashed ? _dashed(path, 10, 7) : path;
    if (style.glow && theme.lineGlow) {
      // Two wide translucent strokes give a soft falloff without a blur filter.
      canvas.drawPath(
        drawn,
        Paint()
          ..color = style.color.withValues(alpha: strong ? 0.25 : 0.12)
          ..strokeWidth = style.width * 4.5
          ..style = PaintingStyle.stroke
          ..strokeCap = StrokeCap.round,
      );
      canvas.drawPath(
        drawn,
        Paint()
          ..color = style.color.withValues(alpha: strong ? 0.45 : 0.25)
          ..strokeWidth = style.width * 2.6
          ..style = PaintingStyle.stroke
          ..strokeCap = StrokeCap.round,
      );
    }
    canvas.drawPath(
      drawn,
      Paint()
        ..color = style.color
        ..strokeWidth = strong ? style.width * 1.6 : style.width
        ..style = PaintingStyle.stroke
        ..strokeCap = StrokeCap.round,
    );
  }

  static Path _dashed(Path path, double on, double off) {
    final out = Path();
    for (final m in path.computeMetrics()) {
      var d = 0.0;
      while (d < m.length) {
        out.addPath(m.extractPath(d, math.min(d + on, m.length)), Offset.zero);
        d += on + off;
      }
    }
    return out;
  }

  void _dots(
    Canvas canvas,
    Path path,
    NetEdgeStyle style,
    double t, {
    required bool backward,
  }) {
    for (final m in path.computeMetrics()) {
      const count = 3;
      for (var i = 0; i < count; i++) {
        // One trip every two seconds, three dots evenly apart.
        var progress = (t / 2 + i / count) % 1.0;
        if (backward) progress = 1 - progress;
        final tangent = m.getTangentForOffset(progress * m.length);
        if (tangent != null) _dot(canvas, tangent.position, style.color, 3.2);
      }
      break;
    }
  }

  void _dot(Canvas canvas, Offset at, Color halo, double radius) {
    if (theme.dotGlow) {
      canvas.drawCircle(
        at,
        radius * 2.6,
        Paint()..color = halo.withValues(alpha: 0.15),
      );
      canvas.drawCircle(
        at,
        radius * 1.7,
        Paint()..color = halo.withValues(alpha: 0.3),
      );
    }
    canvas.drawCircle(at, radius, Paint()..color = theme.dotColor);
  }

  void _pulse(Canvas canvas, Path path, Color color, double progress) {
    for (final m in path.computeMetrics()) {
      final tangent = m.getTangentForOffset(progress * m.length);
      if (tangent == null) return;
      // A short bright comet: a head and a fading tail behind it.
      for (var k = 4; k >= 0; k--) {
        final back = (progress - k * 0.03).clamp(0.0, 1.0);
        final p = m.getTangentForOffset(back * m.length)?.position;
        if (p == null) continue;
        canvas.drawCircle(
          p,
          5.0 - k * 0.7,
          Paint()..color = color.withValues(alpha: 0.9 - k * 0.17),
        );
      }
      _dot(canvas, tangent.position, color, 4.5);
      return;
    }
  }

  void _label(Canvas canvas, Path path, String text, Color color) {
    for (final m in path.computeMetrics()) {
      final at = m.getTangentForOffset(m.length / 2)?.position;
      if (at == null) return;
      final tp = TextPainter(
        text: TextSpan(
          text: text,
          style: TextStyle(
            color: color,
            fontSize: 11,
            fontWeight: FontWeight.w600,
          ),
        ),
        textDirection: TextDirection.ltr,
      )..layout();
      final box = Rect.fromCenter(
        center: at,
        width: tp.width + 10,
        height: tp.height + 4,
      );
      canvas.drawRRect(
        RRect.fromRectAndRadius(box, const Radius.circular(8)),
        Paint()..color = theme.background,
      );
      canvas.drawRRect(
        RRect.fromRectAndRadius(box, const Radius.circular(8)),
        Paint()
          ..color = color
          ..style = PaintingStyle.stroke
          ..strokeWidth = 1,
      );
      tp.paint(canvas, box.topLeft + const Offset(5, 2));
      return;
    }
  }

  @override
  bool shouldRepaint(covariant NetLinkPainter old) =>
      old.data != data ||
      old.positions != positions ||
      old.theme != theme ||
      old.pulses != pulses ||
      old.dragFrom != dragFrom ||
      old.dragTo != dragTo ||
      old.highlight != highlight;
}
