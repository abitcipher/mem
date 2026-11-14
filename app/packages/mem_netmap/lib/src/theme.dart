// Adapted from graph_flow (MIT, Copyright (c) 2025 Benedikt Kotas): the canvas colours of its
// bright and dark presets (grid, node cards, glow and dot colours). See THIRD_PARTY_NOTICES.md.

import 'package:material_ui/material_ui.dart';

/// Colours and effects of the map; [NetMapTheme.of] follows the app's light or dark theme.
@immutable
class NetMapTheme {
  /// Creates a theme.
  const NetMapTheme({
    required this.background,
    required this.gridLine,
    required this.nodeFill,
    required this.nodeBorder,
    required this.nodeShadow,
    required this.text,
    required this.subtleText,
    required this.accent,
    required this.offColor,
    required this.dotColor,
    this.gridSpacing = 48,
    this.lineGlow = true,
    this.dotGlow = true,
  });

  /// Canvas background.
  final Color background;

  /// Grid line colour.
  final Color gridLine;

  /// Distance between grid lines.
  final double gridSpacing;

  /// Node card fill.
  final Color nodeFill;

  /// Node card border.
  final Color nodeBorder;

  /// Node card shadow.
  final Color nodeShadow;

  /// Labels.
  final Color text;

  /// Second lines and faded parts.
  final Color subtleText;

  /// Default node ring colour.
  final Color accent;

  /// Colour of things that are off.
  final Color offColor;

  /// The moving dots' core.
  final Color dotColor;

  /// Glow around links.
  final bool lineGlow;

  /// Glow around moving dots.
  final bool dotGlow;

  /// Dark: graph_flow's dark canvas with neon-style glow.
  static const dark = NetMapTheme(
    background: Color(0xFF17181B),
    gridLine: Color(0x14FFFFFF),
    nodeFill: Color(0xFF2B2D31),
    nodeBorder: Color(0x1FFFFFFF),
    nodeShadow: Color(0x4D000000),
    text: Color(0xFFECEFF1),
    subtleText: Color(0xFF9AA0A6),
    accent: Color(0xFF64FFDA),
    offColor: Color(0xFF5F6368),
    dotColor: Colors.white,
  );

  /// Light: graph_flow's bright canvas, with a softer glow.
  static const light = NetMapTheme(
    background: Color(0xFFF5F5F5),
    gridLine: Color(0x1A616161),
    nodeFill: Colors.white,
    nodeBorder: Color(0x1F000000),
    nodeShadow: Color(0x26000000),
    text: Color(0xFF202124),
    subtleText: Color(0xFF5F6368),
    accent: Color(0xFF00897B),
    offColor: Color(0xFFBDBDBD),
    dotColor: Color(0xFF3E2723),
  );

  /// The theme that matches [context]'s brightness.
  static NetMapTheme of(BuildContext context) =>
      Theme.of(context).brightness == Brightness.dark ? dark : light;
}
