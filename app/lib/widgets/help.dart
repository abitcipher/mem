import 'dart:math' as math;

import 'package:flutter/material.dart';

import '../l10n/gen/app_localizations.dart';

/// An ⓘ button in the app bar that toggles a screen's explanation in a panel over the screen:
/// a tap opens it, a second tap on ⓘ (or a tap outside) closes it. Static text stays out of the
/// way of the codes, the camera and the controls.
class HelpAction extends StatefulWidget {
  const HelpAction({required this.title, required this.text, super.key});

  final String title;
  final String text;

  @override
  State<HelpAction> createState() => _HelpActionState();
}

class _HelpActionState extends State<HelpAction> {
  final _portal = OverlayPortalController();
  final _link = LayerLink();

  /// The button and the panel count as one region: a tap on either is not "outside".
  final _region = Object();

  void _toggle() => setState(_portal.toggle);

  void _hide() {
    if (_portal.isShowing) setState(_portal.hide);
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final theme = Theme.of(context);
    return TapRegion(
      groupId: _region,
      child: CompositedTransformTarget(
        link: _link,
        child: OverlayPortal(
          controller: _portal,
          overlayChildBuilder: (context) {
            final screen = MediaQuery.sizeOf(context);
            return CompositedTransformFollower(
              link: _link,
              targetAnchor: Alignment.bottomRight,
              followerAnchor: Alignment.topRight,
              offset: const Offset(-8, 4),
              child: Align(
                alignment: AlignmentDirectional.topEnd,
                child: TapRegion(
                  groupId: _region,
                  onTapOutside: (_) => _hide(),
                  child: Material(
                    elevation: 6,
                    borderRadius: BorderRadius.circular(12),
                    color: theme.colorScheme.surfaceContainerHigh,
                    child: ConstrainedBox(
                      constraints: BoxConstraints(
                        maxWidth: math.min(screen.width - 32, 400),
                        maxHeight: screen.height * 0.6,
                      ),
                      child: SingleChildScrollView(
                        padding: const EdgeInsets.all(16),
                        child: Column(
                          mainAxisSize: MainAxisSize.min,
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Text(
                              widget.title,
                              style: theme.textTheme.titleSmall,
                            ),
                            const SizedBox(height: 8),
                            Text(widget.text),
                          ],
                        ),
                      ),
                    ),
                  ),
                ),
              ),
            );
          },
          child: IconButton(
            tooltip: l.helpTooltip,
            isSelected: _portal.isShowing,
            icon: const Icon(Icons.info_outline),
            selectedIcon: const Icon(Icons.info),
            onPressed: _toggle,
          ),
        ),
      ),
    );
  }
}
