import 'package:material_ui/material_ui.dart';

import '../app_scope.dart';
import '../l10n/gen/app_localizations.dart';
import '../src/platform.dart';

/// Keeps the background service in step with the setting and the UI language: started
/// at launch when the user turned it on, retitled when the language changes, stopped when off.
class ReachableSync extends StatefulWidget {
  const ReachableSync({required this.child, super.key});

  final Widget child;

  @override
  State<ReachableSync> createState() => _ReachableSyncState();
}

class _ReachableSyncState extends State<ReachableSync> {
  (bool, String, bool)? _applied;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final state = AppScope.of(context);
    final on = state.settings.stayReachable;
    // Bluetooth in the background too.
    final ble = state.node.bleEnabled();
    final l = AppLocalizations.of(context);
    final text = ble ? l.reachableNotificationBle : l.reachableNotification;
    // Nearby radios (Bluetooth, Wi-Fi Direct) add the connected-device service type.
    final radios = ble || state.node.wifiDirectEnabled();
    final want = (on, text, radios);
    // Nothing to stop if it never ran.
    if (want == _applied || (_applied == null && !on)) return;
    _applied = want;
    PlatformServices.setReachable(
      on: on,
      title: l.appTitle,
      text: text,
      ble: radios,
    );
  }

  @override
  Widget build(BuildContext context) => widget.child;
}
