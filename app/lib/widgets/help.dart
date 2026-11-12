import 'dart:math' as math;

import 'package:material_ui/material_ui.dart';

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

/// A setting whose explanation stays hidden until its "?" is tapped; a second tap hides it
/// again. [builder] places the "?" button next to the setting's title; the explanation appears
/// under the setting.
class Explained extends StatefulWidget {
  const Explained({required this.help, required this.builder, super.key});

  final String help;
  final Widget Function(BuildContext context, Widget helpButton) builder;

  @override
  State<Explained> createState() => _ExplainedState();
}

class _ExplainedState extends State<Explained> {
  bool _open = false;

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final reduced = MediaQuery.maybeDisableAnimationsOf(context) ?? false;
    final button = IconButton(
      tooltip: l.helpTooltip,
      isSelected: _open,
      visualDensity: VisualDensity.compact,
      padding: EdgeInsets.zero,
      constraints: const BoxConstraints(minWidth: 36, minHeight: 36),
      iconSize: 20,
      icon: const Icon(Icons.help_outline),
      selectedIcon: const Icon(Icons.help),
      onPressed: () => setState(() => _open = !_open),
    );
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        widget.builder(context, button),
        AnimatedSize(
          duration: reduced ? Duration.zero : const Duration(milliseconds: 150),
          alignment: Alignment.topCenter,
          child: _open
              ? Padding(
                  padding: const EdgeInsets.only(top: 2, bottom: 8),
                  child: Text(
                    widget.help,
                    style: Theme.of(context).textTheme.bodySmall,
                  ),
                )
              : const SizedBox(width: double.infinity),
        ),
      ],
    );
  }
}

/// A setting's title followed by its "?" button.
Widget titleWithHelp(String title, Widget helpButton, {TextStyle? style}) =>
    Row(
      children: [
        Flexible(child: Text(title, style: style)),
        helpButton,
      ],
    );
