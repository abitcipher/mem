import 'dart:math' as math;

import 'package:flutter/material.dart';

import '../theme.dart';

/// The shapes a grade symbol can take. Drawn by the app rather than taken from a font, so every
/// phone shows them and they line up with the digits of the time.
enum SymbolShape {
  /// ⊙ a point.
  dot,

  /// ⊚ a ring.
  ring,

  /// ⊛ an asterisk.
  asterisk,

  /// ⊜ two bars.
  equals,

  /// ⊘ a slash.
  slash,

  /// ⊗ a cross.
  cross,

  /// ◉ a filled centre.
  fisheye,

  /// ⋇ a cross with dots above and below.
  divisionTimes,

  /// ∙ a small filled dot, no circle.
  bullet,
}

/// How grades map to shapes.
enum SymbolStyle {
  /// One shape per grade: ⊙ ⊚ ⊛ ⊜ ⊘ ⊗ for P0 to P5.
  byGrade,

  /// One shape for P0 to P3, chosen by the user; P4 and P5 keep their own.
  oneShape,
}

/// Shapes the user may pick for P0 to P3 under [SymbolStyle.oneShape], in the order offered.
const oneShapeChoices = [
  SymbolShape.fisheye,
  SymbolShape.asterisk,
  SymbolShape.ring,
  SymbolShape.divisionTimes,
  SymbolShape.dot,
  SymbolShape.bullet,
];

/// The privacy grade as a number, 0 to 5; unknown labels count as the least private.
int gradeNumber(String grade) =>
    int.tryParse(grade.startsWith('P') ? grade.substring(1) : grade)
        ?.clamp(0, 5) ??
    5;

/// The shape for [grade]. P4 and P5 never share a shape with P0 to P3, in any style, so the
/// risky grades are recognisable without colour.
SymbolShape shapeFor(String grade, SymbolStyle style, SymbolShape chosen) {
  final n = gradeNumber(grade);
  if (n == 4) return SymbolShape.slash;
  if (n >= 5) return SymbolShape.cross;
  if (style == SymbolStyle.oneShape) return chosen;
  return const [
    SymbolShape.dot,
    SymbolShape.ring,
    SymbolShape.asterisk,
    SymbolShape.equals,
  ][n];
}

/// A grade symbol the size of a digit of [fontSize] text, in the grade's colour for
/// [background].
class GradeSymbol extends StatelessWidget {
  const GradeSymbol({
    required this.grade,
    required this.shape,
    required this.background,
    required this.fontSize,
    super.key,
  });

  final String grade;
  final SymbolShape shape;
  final Color background;
  final double fontSize;

  @override
  Widget build(BuildContext context) {
    final color = MemColors.gradeOn(context, background, grade);
    // A digit is about 0.7 em high; the symbol box matches it and sits on the baseline.
    final side = fontSize * 0.72;
    return SizedBox(
      width: side,
      height: side,
      child: CustomPaint(painter: _SymbolPainter(shape, color)),
    );
  }
}

class _SymbolPainter extends CustomPainter {
  _SymbolPainter(this.shape, this.color);

  final SymbolShape shape;
  final Color color;

  @override
  void paint(Canvas canvas, Size size) {
    final s = size.shortestSide;
    final c = size.center(Offset.zero);
    final r = s / 2;
    final stroke = math.max(1.0, s * 0.11);
    final line = Paint()
      ..color = color
      ..style = PaintingStyle.stroke
      ..strokeWidth = stroke
      ..strokeCap = StrokeCap.round;
    final fill = Paint()
      ..color = color
      ..style = PaintingStyle.fill;
    final outer = r - stroke / 2;
    // Inner marks stay inside the circle with a margin.
    final inner = outer * 0.55;

    if (shape == SymbolShape.bullet) {
      canvas.drawCircle(c, r * 0.4, fill);
      return;
    }
    if (shape != SymbolShape.divisionTimes) {
      canvas.drawCircle(c, outer, line);
    }
    switch (shape) {
      case SymbolShape.dot:
        canvas.drawCircle(c, stroke * 1.1, fill);
      case SymbolShape.ring:
        canvas.drawCircle(c, inner * 0.8, line..strokeWidth = stroke * 0.9);
      case SymbolShape.asterisk:
        for (var i = 0; i < 3; i++) {
          final a = math.pi / 2 + i * math.pi / 3;
          final d = Offset(math.cos(a), math.sin(a)) * inner;
          canvas.drawLine(c - d, c + d, line);
        }
      case SymbolShape.equals:
        final dy = inner * 0.45;
        canvas
          ..drawLine(c + Offset(-inner, -dy), c + Offset(inner, -dy), line)
          ..drawLine(c + Offset(-inner, dy), c + Offset(inner, dy), line);
      case SymbolShape.slash:
        final d = Offset(inner, -inner) * 0.9;
        canvas.drawLine(c - d, c + d, line);
      case SymbolShape.cross:
        final d = inner * 0.8;
        canvas
          ..drawLine(c + Offset(-d, -d), c + Offset(d, d), line)
          ..drawLine(c + Offset(-d, d), c + Offset(d, -d), line);
      case SymbolShape.fisheye:
        canvas.drawCircle(c, inner * 0.85, fill);
      case SymbolShape.divisionTimes:
        final d = r * 0.42;
        canvas
          ..drawLine(c + Offset(-d, -d), c + Offset(d, d), line)
          ..drawLine(c + Offset(-d, d), c + Offset(d, -d), line)
          ..drawCircle(c + Offset(0, -r * 0.82), stroke * 0.8, fill)
          ..drawCircle(c + Offset(0, r * 0.82), stroke * 0.8, fill);
      case SymbolShape.bullet:
        break;
    }
  }

  @override
  bool shouldRepaint(_SymbolPainter old) =>
      old.shape != shape || old.color != color;
}
