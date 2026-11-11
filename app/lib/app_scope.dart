import 'dart:async';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';

import 'l10n/gen/app_localizations.dart';
import 'src/rust/api.dart';
import 'src/backup_schedule.dart';
import 'src/ble.dart';
import 'src/wifi_direct.dart';
import 'src/push.dart';
import 'src/platform.dart';
import 'src/ui_settings.dart';
import 'theme.dart';

/// Holds the Rust node and notifies listeners after every mutation and every node event
/// (messages arriving over Wi-Fi, status changes, contacts coming into range), so screens
/// re-query.
class AppState extends ChangeNotifier {
  AppState(this.node, {this.themeFile, File? backupFile, File? settingsFile})
    : settings = UiSettings.read(settingsFile) {
    _theme = _readTheme();
    unawaited(PlatformServices.setSecureApp(on: settings.blockScreenshots));
    backups = BackupScheduler(node, backupFile, notifyListeners);
    ble = BleBridge(node, reachable: () => settings.stayReachable)..attach();
    wifi = WifiDirectBridge(node, reachable: () => settings.stayReachable)
      ..attach();
    push = PushBridge(node, settings, texts: _pushTexts);
    unawaited(push.attach());
    var contacts = node.contacts().length;
    _events = node.events().listen((e) {
      if (e is NodeEventItem_MessageReceived) {
        _fresh.add(e.contactId);
        _received.add(e.contactId);
      }
      if (e is NodeEventItem_GroupMessage) {
        _fresh.add(e.groupId);
        _received.add(e.groupId);
      }
      if (e is NodeEventItem_DeviceRetired) {
        retired = true;
      }
      if (e is NodeEventItem_Contacts) {
        final now = node.contacts().length;
        if (now > contacts) unawaited(backups.maybeRun(contactAdded: true));
        contacts = now;
      }
      notifyListeners();
    });
  }

  /// UI-only settings; core settings live on [node].
  final UiSettings settings;

  /// Changes UI settings, stores them and redraws.
  void updateSettings(void Function(UiSettings s) change) {
    change(settings);
    settings.write();
    unawaited(PlatformServices.setSecureApp(on: settings.blockScreenshots));
    unawaited(ble.sync());
    unawaited(wifi.sync());
    notifyListeners();
  }

  /// Automatic backups.
  late final BackupScheduler backups;

  /// The Bluetooth radio.
  late final BleBridge ble;
  late final WifiDirectBridge wifi;

  /// Our main device removed this device; Home tells the user and forgets.
  bool retired = false;

  /// Push wake-ups.
  late final PushBridge push;

  /// Wake-up notification text in the chosen language. A headless wake-up never builds the UI,
  /// so it is looked up directly.
  ({String title, String text}) _pushTexts() {
    final locale = switch (settings.language) {
      AppLanguage.en => const Locale('en'),
      AppLanguage.uk => const Locale('uk'),
      AppLanguage.system => ui.PlatformDispatcher.instance.locale,
    };
    final supported = AppLocalizations.supportedLocales.any(
      (l) => l.languageCode == locale.languageCode,
    );
    final l = lookupAppLocalizations(supported ? locale : const Locale('en'));
    return (title: l.appTitle, text: l.pushNotification);
  }

  final MemNode node;

  /// Where the appearance choice is stored; `null` keeps it in memory only (tests).
  final File? themeFile;
  late AppTheme _theme;

  /// The appearance choice; stored in a small file next to the profile (UI-only, not core state).
  AppTheme get theme => _theme;

  set theme(AppTheme value) {
    if (value == _theme) return;
    _theme = value;
    try {
      themeFile?.writeAsStringSync(value.name);
    } on FileSystemException {
      // Keeps working with the in-memory choice.
    }
    notifyListeners();
  }

  AppTheme _readTheme() {
    try {
      final name = themeFile?.readAsStringSync().trim();
      return AppTheme.values.firstWhere(
        (t) => t.name == name,
        orElse: () => AppTheme.system,
      );
    } on FileSystemException {
      return AppTheme.system;
    }
  }

  late final StreamSubscription<NodeEventItem> _events;
  final _received = StreamController<String>.broadcast();

  /// Contact ids of messages that arrived in the background (network channels).
  Stream<String> get received => _received.stream;

  final _fresh = <String>{};

  /// Contacts and groups with messages that arrived since their chat was last opened
  /// (highlighted on Home).
  bool hasFresh(String contactId) => _fresh.contains(contactId);

  /// The user opened this contact's chat.
  void seen(String contactId) {
    if (_fresh.remove(contactId)) notifyListeners();
  }

  /// Call after any call that changes state on the node.
  void changed() => notifyListeners();

  @override
  void dispose() {
    backups.dispose();
    ble.dispose();
    wifi.dispose();
    _events.cancel();
    _received.close();
    super.dispose();
  }
}

class AppScope extends InheritedNotifier<AppState> {
  const AppScope({required AppState state, required super.child, super.key})
    : super(notifier: state);

  static AppState of(BuildContext context) =>
      context.dependOnInheritedWidgetOfExactType<AppScope>()!.notifier!;

  static AppState read(BuildContext context) =>
      context.getInheritedWidgetOfExactType<AppScope>()!.notifier!;
}

/// Current UI locale code passed to Rust for generated disclosures ("en" or "uk").
String localeCode(BuildContext context) =>
    Localizations.localeOf(context).languageCode;

/// Localized text for a Rust reason key or error.
String reasonText(BuildContext context, Object error) {
  final l = AppLocalizations.of(context);
  final key = error is String ? error : error.toString();
  return switch (key) {
    'malformed' => l.reason_malformed,
    'unsupported' => l.reason_unsupported,
    'not_for_us' => l.reason_not_for_us,
    'undecryptable' => l.reason_undecryptable,
    'bad_signature' => l.reason_bad_signature,
    'duplicate' => l.reason_duplicate,
    'purged' => l.reason_purged,
    'expired' => l.reason_expired,
    'own_card' => l.reason_own_card,
    'unknown_contact' => l.reason_unknown_contact,
    'unknown_message' => l.reason_unknown_message,
    'too_large_for_channel' => l.reason_too_large_for_channel,
    'consent_mismatch' => l.reason_consent_mismatch,
    'invalid_text' => l.reason_invalid_text,
    'invalid_name' => l.reason_invalid_name,
    'unreachable' => l.reason_unreachable,
    'channel_disabled' => l.reason_channel_disabled,
    'quota' => l.reason_quota,
    'invalid_floor' => l.reason_invalid_floor,
    'no_pending_phrase' => l.phraseNotConfirmed,
    'waiting_for_key' => l.reason_waiting_for_key,
    'awaiting_approval' => l.reason_awaiting_approval,
    'bad_proof_of_work' => l.reason_bad_proof_of_work,
    'unknown_request' => l.reason_unknown_request,
    'already_contact' => l.reason_already_contact,
    'invalid_note' => l.reason_invalid_note,
    'not_a_card' => l.reason_not_a_card,
    'main_device_only' => l.reason_main_device_only,
    'too_many_devices' => l.reason_too_many_devices,
    'device_known' => l.reason_device_known,
    'unknown_device' => l.reason_unknown_device,
    'not_empty' => l.reason_not_empty,
    'not_for_this_device' => l.reason_not_for_this_device,
    'no_local_network' => l.reason_no_local_network,
    'unknown_group' => l.reason_unknown_group,
    'not_group_admin' => l.reason_not_group_admin,
    'group_full' => l.reason_group_full,
    'group_empty' => l.reason_group_empty,
    'not_a_contact' => l.reason_not_a_contact,
    'invalid_hub_url' => l.reason_invalid_hub_url,
    'not_plain_text' => l.reason_not_plain_text,
    'invalid_email_account' => l.reason_invalid_email_account,
    'invalid_email_address' => l.reason_invalid_email_address,
    _ => l.error(key),
  };
}

String gradeLabel(AppLocalizations l, int grade) => switch (grade) {
  0 => l.grade0,
  1 => l.grade1,
  2 => l.grade2,
  3 => l.grade3,
  4 => l.grade4,
  _ => l.grade5,
};

String channelLabel(AppLocalizations l, Channel c) => switch (c) {
  Channel.qr => l.channelQr,
  Channel.lan => l.channelLan,
  Channel.ble => l.channelBle,
  Channel.hub => l.channelHub,
  Channel.plain => l.channelPlain,
  Channel.email => l.channelEmail,
  Channel.share => l.channelShare,
  Channel.courier => l.channelCourier,
  Channel.wifiDirect => l.channelWifiDirect,
  Channel.audio => l.channelAudio,
};

/// The label of a channel as it was used: a hub message that went through (or may go
/// through) partner operators' hubs is P3 and says so.
String usedChannelLabel(AppLocalizations l, Channel c, String grade) =>
    c == Channel.hub && grade == 'P3'
    ? l.channelHubPartners
    : channelLabel(l, c);

IconData channelIcon(Channel c) => switch (c) {
  Channel.qr => Icons.qr_code_2,
  Channel.lan => Icons.wifi,
  Channel.ble => Icons.bluetooth,
  Channel.hub => Icons.cloud_outlined,
  Channel.plain => Icons.lock_open,
  Channel.email => Icons.alternate_email,
  Channel.share => Icons.ios_share,
  Channel.courier => Icons.directions_walk,
  Channel.wifiDirect => Icons.wifi_tethering,
  Channel.audio => Icons.graphic_eq,
};

String contactTitle(ContactItem c) =>
    c.displayName ?? c.fingerprint.split(' ').take(2).join(' ');

/// An alert as text, with an optional coloured icon and an optional action (never a modal
/// window for something that needs no decision).
void showSnack(
  BuildContext context,
  String text, {
  IconData? icon,
  Color? iconColor,
  SnackBarAction? action,
}) {
  ScaffoldMessenger.of(context)
    ..hideCurrentSnackBar()
    ..showSnackBar(
      SnackBar(
        content: icon == null
            ? Text(text)
            : Row(
                children: [
                  Icon(icon, color: iconColor),
                  const SizedBox(width: 12),
                  Expanded(child: Text(text)),
                ],
              ),
        action: action,
        // Flutter keeps a snack bar with an action until it is used; ours are hints and must
        // never stay over the screen (they cover the chat's text field).
        persist: false,
      ),
    );
}
