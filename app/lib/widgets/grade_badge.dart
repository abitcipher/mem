import 'package:flutter/material.dart';

import '../theme.dart';

/// Compact provenance badge, e.g. "QR · P0". Coloured by grade, and distinguishable without
/// colour too: thin outline up to P3, heavier for P4, filled for P5 (plaintext).
class GradeBadge extends StatelessWidget {
  const GradeBadge({
    required this.label,
    required this.grade,
    this.detail,
    super.key,
  });

  final String label;
  final String grade;

  /// Extra text after the grade, e.g. the time the channel carried the message.
  final String? detail;

  @override
  Widget build(BuildContext context) {
    final color = MemColors.grade(context, grade);
    final plaintext = grade == 'P5';
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 1),
      decoration: BoxDecoration(
        color: plaintext ? color : null,
        border: Border.all(color: color, width: grade == 'P4' ? 2 : 1),
        borderRadius: BorderRadius.circular(8),
      ),
      child: Text(
        detail == null ? '$label · $grade' : '$label · $grade · $detail',
        style: TextStyle(
          fontSize: 11,
          color: plaintext ? Theme.of(context).colorScheme.onError : color,
          fontWeight: FontWeight.w600,
        ),
      ),
    );
  }
}
