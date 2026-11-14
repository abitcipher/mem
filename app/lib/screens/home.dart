import 'dart:async';

import 'package:material_ui/material_ui.dart';
import 'package:intl/intl.dart';

import '../app_scope.dart';
import '../src/rust/api.dart';
import '../l10n/gen/app_localizations.dart';
import 'channels.dart';
import 'nearby.dart';
import 'chat.dart';
import 'groups.dart';
import 'pair.dart';
import 'privacy_log.dart';
import 'settings.dart';
import 'shares.dart';
import 'receive.dart';
import 'requests.dart';
import '../widgets/logo.dart';
import '../widgets/glow.dart';
import '../widgets/logo_mark.dart';
import '../widgets/inbound.dart';
import 'attest.dart';
import 'backup.dart';
import 'devices.dart';

class HomeScreen extends StatelessWidget {
  const HomeScreen({super.key});

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final state = AppScope.of(context);
    final contacts = state.node.contacts();
    final nearby = state.node.nearby().toSet();
    final lanOn = state.node.lanEnabled();
    final devices = state.node.nearbyDevices();
    final groups = state.node.groups();
    final invitations = groups
        .where((g) => g.status == GroupStatusItem.invited)
        .length;
    final shown = groups
        .where((g) => g.status != GroupStatusItem.invited)
        .toList();
    final requests = state.node.requests().length + invitations;
    void push(Widget w) =>
        Navigator.of(context).push(MaterialPageRoute<void>(builder: (_) => w));
    // Our main device removed this device: say so once, then forget.
    if (state.retired) {
      state.retired = false;
      WidgetsBinding.instance.addPostFrameCallback(
        (_) => unawaited(showDeviceRetired(context)),
      );
    }
    return InboundText(
      child: Scaffold(
        appBar: AppBar(
          title: Text(l.conversations),
          actions: [
            if (state.settings.showNetworkStatus)
              IconButton(
                tooltip: lanOn ? l.nearbyDevices(devices) : l.lanOffShort,
                icon: RainbowGlow(
                  active: lanOn && devices > 0,
                  circle: true,
                  child: Padding(
                    padding: const EdgeInsets.all(4),
                    child: Icon(
                      !lanOn
                          ? Icons.wifi_off
                          : devices > 0
                          ? Icons.wifi
                          : Icons.wifi_find,
                    ),
                  ),
                ),
                onPressed: () => push(const NearbyScreen()),
              ),
            // Only while someone is waiting for an answer (requests are not messages).
            if (requests > 0)
              IconButton(
                tooltip: l.requestsTooltip(requests),
                icon: Badge.count(
                  count: requests,
                  child: const Icon(Icons.person_search_outlined),
                ),
                onPressed: () => push(const RequestsScreen()),
              ),
            // Minimal home: everything that is not needed every time lives in one menu.
            _MainMenu(
              onSelected: push,
              itemBuilder: (_) => [
                _menuItem(
                  Icons.qr_code_scanner,
                  l.receive,
                  const ReceiveScreen(),
                ),
                if (contacts.isNotEmpty)
                  _menuItem(
                    Icons.group_add_outlined,
                    l.newGroup,
                    const CreateGroupScreen(),
                  ),
                if (contacts.isNotEmpty)
                  _menuItem(
                    Icons.folder_outlined,
                    l.sharedWithMe,
                    const SharedWithMeScreen(),
                  ),
                _menuItem(
                  Icons.handshake_outlined,
                  l.attestTitle,
                  const AttestScreen(),
                ),
                _menuItem(
                  Icons.policy_outlined,
                  l.privacyLog,
                  const PrivacyLogScreen(),
                ),
                _menuItem(
                  Icons.hub_outlined,
                  l.channels,
                  const ChannelsScreen(),
                ),
                _menuItem(Icons.devices, l.devices, const DevicesScreen()),
                _menuItem(
                  Icons.settings_outlined,
                  l.settings,
                  const SettingsScreen(),
                ),
              ],
            ),
          ],
        ),
        // Icon only; the tooltip names it on hover and long-press, and screen readers read it.
        floatingActionButton: FloatingActionButton(
          heroTag: 'addContact',
          tooltip: l.addContact,
          onPressed: () => push(const PairScreen()),
          child: const Icon(Icons.person_add_alt),
        ),
        body: contacts.isEmpty && shown.isEmpty
            ? Center(
                child: Padding(
                  padding: const EdgeInsets.all(32),
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      const MemLogo(size: 72),
                      const SizedBox(height: 16),
                      Text(
                        l.noContactsTitle,
                        style: Theme.of(context).textTheme.titleLarge,
                      ),
                      const SizedBox(height: 8),
                      Text(l.noContactsBody, textAlign: TextAlign.center),
                      const SizedBox(height: 16),
                      TextButton.icon(
                        icon: const Icon(Icons.restore),
                        label: Text(l.restoreTitle),
                        onPressed: () => push(const RestoreScreen()),
                      ),
                      TextButton.icon(
                        icon: const Icon(Icons.devices),
                        label: Text(l.joinAsDevice),
                        onPressed: () => push(const JoinDeviceScreen()),
                      ),
                    ],
                  ),
                ),
              )
            : ListView.separated(
                itemCount: shown.length + contacts.length,
                separatorBuilder: (_, _) => const Divider(height: 1),
                itemBuilder: (context, i) {
                  if (i < shown.length) {
                    final g = shown[i];
                    final joined = g.members
                        .where((m) => m.role != MemberRoleItem.invited)
                        .length;
                    return Padding(
                      padding: const EdgeInsets.symmetric(
                        horizontal: 4,
                        vertical: 2,
                      ),
                      child: RainbowGlow(
                        active: state.hasFresh(g.id),
                        child: ListTile(
                          leading: const CircleAvatar(
                            child: Icon(Icons.group_outlined),
                          ),
                          title: Text(g.name),
                          subtitle: Text(
                            switch (g.status) {
                              GroupStatusItem.joining => l.groupJoining,
                              GroupStatusItem.left => l.groupLeftBanner,
                              GroupStatusItem.removed => l.groupRemovedBanner,
                              _ => l.groupMembersCount(joined),
                            },
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                          ),
                          onTap: () {
                            state.seen(g.id);
                            push(GroupChatScreen(groupId: g.id));
                          },
                        ),
                      ),
                    );
                  }
                  final c = contacts[i - shown.length];
                  final title = contactTitle(c);
                  final tile = ListTile(
                    leading: RainbowGlow(
                      active: nearby.contains(c.id),
                      circle: true,
                      child: CircleAvatar(
                        child: Text(title.characters.first.toUpperCase()),
                      ),
                    ),
                    title: Row(
                      children: [
                        Flexible(child: Text(title)),
                        if (nearby.contains(c.id)) ...[
                          const SizedBox(width: 6),
                          Icon(
                            Icons.near_me,
                            size: 16,
                            semanticLabel: l.nearby,
                          ),
                        ],
                      ],
                    ),
                    subtitle: Text(
                      c.requested
                          ? l.requestedSubtitle
                          : c.lastText ??
                                (c.metInPerson ? l.metInPerson : l.remote),
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    ),
                    trailing: Text(
                      DateFormat.MMMd(
                        Localizations.localeOf(context).toString(),
                      ).format(
                        DateTime.fromMillisecondsSinceEpoch(
                          c.lastUnixMs.toInt(),
                        ),
                      ),
                    ),
                    onTap: () {
                      state.seen(c.id);
                      push(ChatScreen(contactId: c.id));
                    },
                  );
                  // A new message is an event: outline the row until its chat is opened.
                  return Padding(
                    padding: const EdgeInsets.symmetric(
                      horizontal: 4,
                      vertical: 2,
                    ),
                    child: RainbowGlow(
                      active: state.hasFresh(c.id),
                      child: tile,
                    ),
                  );
                },
              ),
      ),
    );
  }

  static PopupMenuItem<Widget> _menuItem(
    IconData icon,
    String label,
    Widget screen,
  ) => PopupMenuItem<Widget>(
    value: screen,
    child: ListTile(
      leading: Icon(icon),
      title: Text(label),
      contentPadding: EdgeInsets.zero,
    ),
  );
}

/// The main menu behind the MEM logo: the logo turns half a turn clockwise when the menu opens
/// and another half turn clockwise when it closes, back upright.
class _MainMenu extends StatefulWidget {
  const _MainMenu({required this.onSelected, required this.itemBuilder});

  final void Function(Widget screen) onSelected;
  final PopupMenuItemBuilder<Widget> itemBuilder;

  @override
  State<_MainMenu> createState() => _MainMenuState();
}

class _MainMenuState extends State<_MainMenu> {
  var _halfTurns = 0;

  void _turn() => setState(() => _halfTurns++);

  @override
  Widget build(BuildContext context) => PopupMenuButton<Widget>(
    icon: SpinningLogo(halfTurns: _halfTurns),
    // Below the button, so the logo stays visible while it turns.
    position: PopupMenuPosition.under,
    onOpened: _turn,
    onCanceled: _turn,
    onSelected: (screen) {
      _turn();
      widget.onSelected(screen);
    },
    itemBuilder: widget.itemBuilder,
  );
}
