import 'package:material_ui/material_ui.dart';

/// The MEM logo as a vector, drawn from `assets/brand/logo.svg` (its paths in the SVG's own
/// 61.56-unit square): a disc in [color] with "m!" cut out and filled with [cutout], and the
/// ring around the dot of the "!". Sharp at any size, and fits light and dark themes.
class LogoMark extends StatelessWidget {
  /// Creates the mark; colours default to the icon colour and the surface.
  const LogoMark({this.size = 24, this.color, this.cutout, super.key});

  /// Width and height.
  final double size;

  /// The disc; the current icon colour when null.
  final Color? color;

  /// The letters; the surface colour when null.
  final Color? cutout;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final disc = color ?? IconTheme.of(context).color ?? scheme.onSurface;
    return SizedBox.square(
      dimension: size,
      child: CustomPaint(
        painter: _LogoPainter(disc: disc, cutout: cutout ?? scheme.surface),
      ),
    );
  }
}

/// The menu button's icon: the logo turning half a turn clockwise each time the menu opens or
/// closes, so open is upside down and closed is upright again.
class SpinningLogo extends StatelessWidget {
  /// Creates the icon turned by [halfTurns] half turns.
  const SpinningLogo({required this.halfTurns, this.size = 26, super.key});

  /// Menu opens and closes so far.
  final int halfTurns;

  /// Width and height.
  final double size;

  @override
  Widget build(BuildContext context) => AnimatedRotation(
    turns: halfTurns / 2,
    duration: const Duration(milliseconds: 350),
    curve: Curves.easeInOutCubic,
    child: LogoMark(size: size),
  );
}

class _LogoPainter extends CustomPainter {
  _LogoPainter({required this.disc, required this.cutout});

  final Color disc;
  final Color cutout;

  /// Side of the SVG's view box.
  static const _view = 61.556438;

  @override
  void paint(Canvas canvas, Size size) {
    canvas.scale(size.width / _view, size.height / _view);
    // The letters show the light disc behind the dark one, as in the SVG.
    canvas.drawCircle(
      const Offset(30.9415, 30.9415),
      25.191498,
      Paint()..color = cutout,
    );
    canvas.drawPath(_path3(), Paint()..color = Color.lerp(cutout, disc, 0.15)!);
    canvas.drawPath(_path17(), Paint()..color = disc);
  }

  @override
  bool shouldRepaint(covariant _LogoPainter old) =>
      old.disc != disc || old.cutout != cutout;
}

// Paths of assets/brand/logo.svg, moved by the layer's translate(-74.221786,-117.72178).
// path3 #e3e2db
Path _path3() => Path()
  ..fillType = PathFillType.evenOdd
  ..moveTo(43.7767, 11.9911)
  ..arcToPoint(
    Offset(39.7697, 15.9985),
    radius: Radius.elliptical(4.007, 4.007),
    rotation: 0,
    largeArc: false,
    clockwise: false,
  )
  ..arcToPoint(
    Offset(43.7767, 20.0054),
    radius: Radius.elliptical(4.007, 4.007),
    rotation: 0,
    largeArc: false,
    clockwise: false,
  )
  ..arcToPoint(
    Offset(47.7836, 15.9985),
    radius: Radius.elliptical(4.007, 4.007),
    rotation: 0,
    largeArc: false,
    clockwise: false,
  )
  ..arcToPoint(
    Offset(43.7767, 11.9911),
    radius: Radius.elliptical(4.007, 4.007),
    rotation: 0,
    largeArc: false,
    clockwise: false,
  )
  ..close()
  ..moveTo(43.7767, 12.6795)
  ..arcToPoint(
    Offset(47.0952, 15.9985),
    radius: Radius.elliptical(3.3186, 3.3186),
    rotation: 0,
    largeArc: false,
    clockwise: true,
  )
  ..arcToPoint(
    Offset(43.7767, 19.317),
    radius: Radius.elliptical(3.3186, 3.3186),
    rotation: 0,
    largeArc: false,
    clockwise: true,
  )
  ..arcToPoint(
    Offset(40.4582, 15.9985),
    radius: Radius.elliptical(3.3186, 3.3186),
    rotation: 0,
    largeArc: false,
    clockwise: true,
  )
  ..arcToPoint(
    Offset(43.7767, 12.6795),
    radius: Radius.elliptical(3.3186, 3.3186),
    rotation: 0,
    largeArc: false,
    clockwise: true,
  )
  ..close();

// path17 #4d4d4d
Path _path17() => Path()
  ..fillType = PathFillType.evenOdd
  ..moveTo(30.9413, 4.2943)
  ..arcToPoint(
    Offset(4.2943, 30.9413),
    radius: Radius.elliptical(26.6469, 26.6469),
    rotation: 0,
    largeArc: false,
    clockwise: false,
  )
  ..arcToPoint(
    Offset(30.9413, 57.5882),
    radius: Radius.elliptical(26.6469, 26.6469),
    rotation: 0,
    largeArc: false,
    clockwise: false,
  )
  ..arcToPoint(
    Offset(57.5882, 30.9413),
    radius: Radius.elliptical(26.6469, 26.6469),
    rotation: 0,
    largeArc: false,
    clockwise: false,
  )
  ..arcToPoint(
    Offset(30.9413, 4.2943),
    radius: Radius.elliptical(26.6469, 26.6469),
    rotation: 0,
    largeArc: false,
    clockwise: false,
  )
  ..close()
  ..moveTo(43.7529, 11.7099)
  ..cubicTo(46.1215, 11.7098, 48.0416, 13.6299, 48.0415, 15.9985)
  ..cubicTo(48.0416, 18.3671, 46.1215, 20.2872, 43.7529, 20.2871)
  ..cubicTo(41.3843, 20.2872, 39.4642, 18.3671, 39.4643, 15.9985)
  ..cubicTo(39.4642, 13.6299, 41.3843, 11.7098, 43.7529, 11.7099)
  ..close()
  ..moveTo(18.3586, 13.8555)
  ..cubicTo(20.9587, 13.9637, 22.626, 15.6931, 22.626, 18.1224)
  ..lineTo(22.626, 39.2555)
  ..cubicTo(22.626, 40.3987, 24.0317, 41.4284, 25.3928, 41.3892)
  ..cubicTo(26.0526, 41.3634, 26.6275, 41.0886, 27.0366, 40.6327)
  ..cubicTo(27.2054, 40.4445, 27.3458, 40.2257, 27.4526, 39.9805)
  ..lineTo(27.4526, 39.7826)
  ..lineTo(27.4526, 39.5841)
  ..lineTo(27.4526, 38.9087)
  ..lineTo(27.4526, 24.5613)
  ..cubicTo(28.017, 23.4518, 28.8489, 22.3392, 31.0586, 22.3392)
  ..cubicTo(33.6506, 22.4445, 34.7956, 24.0881, 35.0201, 26.5085)
  ..lineTo(35.0201, 39.2085)
  ..cubicTo(35.2376, 40.3837, 36.1466, 41.3371, 37.3579, 41.3892)
  ..cubicTo(38.6818, 41.3341, 39.5423, 40.4968, 39.5423, 39.2555)
  ..lineTo(39.5423, 29.4845)
  ..cubicTo(39.5423, 27.126, 40.9377, 24.5727, 43.5529, 24.6569)
  ..cubicTo(46.1681, 24.7411, 47.5589, 27.0538, 47.5589, 29.4845)
  ..lineTo(47.5589, 43.5224)
  ..cubicTo(47.1802, 45.898, 46.2058, 47.8031, 43.5555, 47.7392)
  ..lineTo(32.9034, 47.7392)
  ..cubicTo(32.3388, 46.3644, 31.7741, 44.9897, 31.2095, 43.6149)
  ..cubicTo(30.1482, 44.9897, 29.0867, 46.3642, 28.0257, 47.7392)
  ..lineTo(26.2516, 47.7392)
  ..lineTo(18.3586, 47.7392)
  ..cubicTo(15.8151, 47.6338, 14.3755, 45.9557, 14.1423, 43.5224)
  ..lineTo(14.1423, 18.1224)
  ..cubicTo(14.1423, 15.7445, 15.6879, 13.7949, 18.3586, 13.8555)
  ..close();
