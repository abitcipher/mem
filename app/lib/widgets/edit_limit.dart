import 'package:material_ui/material_ui.dart';

import '../l10n/gen/app_localizations.dart';

/// Positions of the edit-limit scale: 0 always, 1..11 a duration, 12 never.
const editLimitPositions = 13;

/// The label of edit-limit [position].
String editLimitLabel(AppLocalizations l, int position) => switch (position) {
  0 => l.editLimitAlways,
  1 => l.durationMinutes(5),
  2 => l.durationMinutes(15),
  3 => l.durationMinutes(30),
  4 => l.durationMinutes(60),
  5 => l.durationHours(2),
  6 => l.durationHours(8),
  7 => l.durationHours(12),
  8 => l.durationHours(24),
  9 => l.durationDays(5),
  10 => l.durationDays(7),
  11 => l.durationDays(31),
  _ => l.editLimitNever,
};

/// A slider over the edit-limit scale, its value shown above it.
class EditLimitSlider extends StatelessWidget {
  const EditLimitSlider({
    required this.position,
    required this.onChanged,
    super.key,
  });

  final int position;
  final ValueChanged<int> onChanged;

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(
          editLimitLabel(l, position),
          style: Theme.of(context).textTheme.bodyLarge,
        ),
        Slider(
          value: position.toDouble(),
          max: (editLimitPositions - 1).toDouble(),
          divisions: editLimitPositions - 1,
          label: editLimitLabel(l, position),
          onChanged: (v) => onChanged(v.round()),
        ),
      ],
    );
  }
}
