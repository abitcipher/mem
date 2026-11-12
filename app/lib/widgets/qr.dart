import 'package:material_ui/material_ui.dart';

import '../app_scope.dart';

import '../src/platform.dart';
import '../src/rust/api.dart';

/// Keeps the screen at full brightness and awake while [child] is on screen, for showing QR
/// codes to another camera.
class ShowingCode extends StatefulWidget {
  const ShowingCode({required this.child, super.key});

  final Widget child;

  @override
  State<ShowingCode> createState() => _ShowingCodeState();
}

class _ShowingCodeState extends State<ShowingCode> {
  @override
  void initState() {
    super.initState();
    PlatformServices.setCodeBrightness(
      AppScope.read(context).settings.codeBrightness,
    );
    PlatformServices.showingCode(on: true);
  }

  @override
  void dispose() {
    PlatformServices.showingCode(on: false);
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => widget.child;
}

/// Draws a QR bit matrix from Rust (no image encoding in Dart).
///
/// The code fits in a square of at most [size] logical pixels, including the 4-module white
/// quiet zone that QR readers need. Each module is a whole number of physical pixels, so every
/// module has the same width and its edges stay sharp for the other camera.
class QrMatrix extends StatelessWidget {
  const QrMatrix({required this.qr, this.size = 300, super.key});

  /// Quiet zone on each side, in modules (ISO/IEC 18004).
  static const quietModules = 4;

  final QrView qr;
  final double size;

  @override
  Widget build(BuildContext context) {
    final dpr = MediaQuery.devicePixelRatioOf(context);
    final modules = qr.width + 2 * quietModules;
    final modulePx = (size * dpr / modules).floorToDouble().clamp(1.0, 1e6);
    final side = modulePx * modules / dpr;
    return SizedBox.square(
      dimension: side,
      child: CustomPaint(painter: _QrPainter(qr, modulePx / dpr)),
    );
  }
}

class _QrPainter extends CustomPainter {
  _QrPainter(this.qr, this.cell);

  final QrView qr;

  /// One module in logical pixels (a whole number of physical pixels).
  final double cell;

  @override
  void paint(Canvas canvas, Size size) {
    canvas.drawRect(Offset.zero & size, Paint()..color = Colors.white);
    final dark = Path();
    const q = QrMatrix.quietModules;
    for (var y = 0; y < qr.width; y++) {
      for (var x = 0; x < qr.width; x++) {
        if (qr.modules[y * qr.width + x] == 1) {
          dark.addRect(
            Rect.fromLTWH((x + q) * cell, (y + q) * cell, cell, cell),
          );
        }
      }
    }
    canvas.drawPath(
      dark,
      Paint()
        ..color = Colors.black
        ..isAntiAlias = false,
    );
  }

  @override
  bool shouldRepaint(_QrPainter old) => old.qr != qr || old.cell != cell;
}
