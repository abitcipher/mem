import 'package:flutter/material.dart';

/// The MEM logo (assets/brand/logo.svg, rendered with the "m!" filled white so it reads the same
/// on light and dark backgrounds). Decorative: always next to text that names the screen.
class MemLogo extends StatelessWidget {
  const MemLogo({this.size = 48, super.key});

  final double size;

  @override
  Widget build(BuildContext context) => Image.asset(
    'assets/brand/logo.png',
    width: size,
    height: size,
    filterQuality: FilterQuality.medium,
    excludeFromSemantics: true,
  );
}
