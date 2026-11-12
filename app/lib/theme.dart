import 'package:material_ui/material_ui.dart';

/// The user's appearance choice (Profile → Appearance).
enum AppTheme { system, white, gray, black }

/// Neutral theme: buttons, icons and surfaces in greys, in three bases — white, gray and black.
/// Colour is kept for the few elements where it carries meaning (grade badges, exposure markers,
/// errors); highlights use the rainbow outline in `widgets/glow.dart`. The logo keeps its orange.
ThemeData memTheme(AppTheme base) {
  final dark = base == AppTheme.gray || base == AppTheme.black;
  var scheme = ColorScheme.fromSeed(
    seedColor: const Color(0xFF808080),
    brightness: dark ? Brightness.dark : Brightness.light,
    dynamicSchemeVariant: DynamicSchemeVariant.monochrome,
  );
  if (base == AppTheme.gray) {
    scheme = scheme.copyWith(
      surface: const Color(0xFF3A3A3C),
      surfaceContainerLowest: const Color(0xFF323234),
      surfaceContainerLow: const Color(0xFF404042),
      surfaceContainer: const Color(0xFF464648),
      surfaceContainerHigh: const Color(0xFF4E4E50),
      surfaceContainerHighest: const Color(0xFF58585A),
    );
  } else if (base == AppTheme.black) {
    scheme = scheme.copyWith(
      surface: const Color(0xFF000000),
      surfaceContainerLowest: const Color(0xFF000000),
      surfaceContainerLow: const Color(0xFF0C0C0C),
      surfaceContainer: const Color(0xFF141414),
      surfaceContainerHigh: const Color(0xFF1C1C1C),
      surfaceContainerHighest: const Color(0xFF262626),
    );
  }
  return ThemeData(colorScheme: scheme, useMaterial3: true);
}

/// Meaningful colours, tuned for contrast on light or dark bases.
abstract final class MemColors {
  static bool _dark(BuildContext context) =>
      Theme.of(context).brightness == Brightness.dark;

  /// Provenance grade colour: green direct, blue-grey own infrastructure, orange foreign
  /// platform, red plaintext.
  static Color grade(BuildContext context, String grade) {
    final d = _dark(context);
    return switch (grade) {
      'P0' || 'P1' => d ? Colors.green.shade300 : Colors.green.shade700,
      'P2' || 'P3' => d ? Colors.blueGrey.shade200 : Colors.blueGrey.shade600,
      'P4' => d ? Colors.orange.shade300 : Colors.orange.shade800,
      _ => Theme.of(context).colorScheme.error,
    };
  }

  /// The grade colour for something drawn on [background] (a message bubble): the light shade
  /// on a dark background and the dark shade on a light one, so the grade keeps its contrast on
  /// both bubble colours in both themes.
  static Color gradeOn(BuildContext context, Color background, String grade) {
    final darkBg = background.computeLuminance() < 0.4;
    return switch (grade) {
      'P0' || 'P1' => darkBg ? Colors.green.shade300 : Colors.green.shade800,
      'P2' ||
      'P3' => darkBg ? Colors.blueGrey.shade200 : Colors.blueGrey.shade700,
      'P4' => darkBg ? Colors.orange.shade300 : Colors.orange.shade900,
      _ => darkBg ? Colors.red.shade300 : Colors.red.shade800,
    };
  }

  /// Muted text on [background] (a message bubble).
  static Color onBubble(Color background) => background.computeLuminance() < 0.4
      ? Colors.white.withValues(alpha: 0.75)
      : Colors.black.withValues(alpha: 0.65);

  /// "They will learn" marker in disclosures.
  static Color exposed(BuildContext context) =>
      _dark(context) ? Colors.orange.shade300 : Colors.orange.shade800;

  /// "They may partly learn" marker.
  static Color limited(BuildContext context) =>
      _dark(context) ? Colors.amber.shade200 : Colors.amber.shade800;

  /// "Stays private" marker.
  static Color protected(BuildContext context) =>
      _dark(context) ? Colors.green.shade300 : Colors.green.shade700;
}
