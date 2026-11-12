import 'dart:math' as math;

import 'package:material_ui/material_ui.dart';

/// The highlight of the neutral UI: a hairline outline with a faint rainbow glow, drawn along part
/// of the edge only and fading into the background at both ends, so it is barely noticeable.
/// It marks events (a new message, a contact nearby) and the recommended choice. It is never used
/// for warnings.
///
/// It is static by default. Pass [animate] only while an interactive process runs (a QR transfer
/// being shown or received); the arc then travels slowly around the edge, unless the platform asks
/// for reduced motion. When [active] is false only [child] is laid out.
class RainbowGlow extends StatefulWidget {
  const RainbowGlow({
    required this.child,
    this.active = true,
    this.animate = false,
    this.circle = false,
    this.radius = 12,
    super.key,
  });

  final Widget child;
  final bool active;
  final bool animate;

  /// Outline a circle (avatars, icons) instead of a rounded rectangle.
  final bool circle;
  final double radius;

  @override
  State<RainbowGlow> createState() => _RainbowGlowState();
}

class _RainbowGlowState extends State<RainbowGlow>
    with SingleTickerProviderStateMixin {
  late final AnimationController _spin = AnimationController(
    vsync: this,
    duration: const Duration(seconds: 4),
  );

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _sync();
  }

  @override
  void didUpdateWidget(RainbowGlow old) {
    super.didUpdateWidget(old);
    _sync();
  }

  void _sync() {
    final still = MediaQuery.maybeDisableAnimationsOf(context) ?? false;
    if (widget.active && widget.animate && !still) {
      if (!_spin.isAnimating) _spin.repeat();
    } else {
      _spin.stop();
    }
  }

  @override
  void dispose() {
    _spin.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    if (!widget.active) return widget.child;
    return CustomPaint(
      foregroundPainter: _GlowPainter(
        spin: _spin,
        circle: widget.circle,
        radius: widget.radius,
        // One physical pixel, but never thinner than half a logical pixel.
        width: math.max(1 / MediaQuery.devicePixelRatioOf(context), 0.5),
      ),
      child: widget.child,
    );
  }
}

class _GlowPainter extends CustomPainter {
  _GlowPainter({
    required this.spin,
    required this.circle,
    required this.radius,
    required this.width,
  }) : super(repaint: spin);

  final Animation<double> spin;
  final bool circle;
  final double radius;
  final double width;

  // The visible arc covers about 60 % of the perimeter and fades to transparent at both ends,
  // so it blends into the white, gray or black base.
  static const _colors = [
    Color(0x00FF3B30),
    Color(0xFFFF3B30),
    Color(0xFFFF9500),
    Color(0xFFFFCC00),
    Color(0xFF34C759),
    Color(0xFF007AFF),
    Color(0xFFAF52DE),
    Color(0x00AF52DE),
    Color(0x00AF52DE),
  ];
  static const _stops = [0.0, 0.08, 0.18, 0.28, 0.38, 0.5, 0.6, 0.68, 1.0];

  @override
  void paint(Canvas canvas, Size size) {
    final rect = Offset.zero & size;
    // Static arcs start at the top left; animated ones travel around.
    final angle = -math.pi * 0.85 + spin.value * 2 * math.pi;
    final shader = SweepGradient(
      colors: _colors,
      stops: _stops,
      transform: GradientRotation(angle),
    ).createShader(rect);
    final shape = circle
        ? (Path()..addOval(rect.deflate(width / 2)))
        : (Path()..addRRect(
            RRect.fromRectAndRadius(
              rect.deflate(width / 2),
              Radius.circular(radius),
            ),
          ));
    // A faint glow under the line, then the line itself.
    canvas.drawPath(
      shape,
      Paint()
        ..shader = shader
        ..style = PaintingStyle.stroke
        ..strokeWidth = width * 4
        ..color = const Color(0x40000000)
        ..maskFilter = MaskFilter.blur(BlurStyle.normal, width * 3),
    );
    canvas.drawPath(
      shape,
      Paint()
        ..shader = shader
        ..style = PaintingStyle.stroke
        ..strokeWidth = width
        ..color = const Color(0xCC000000),
    );
  }

  @override
  bool shouldRepaint(_GlowPainter old) =>
      old.circle != circle || old.radius != radius || old.width != width;
}
