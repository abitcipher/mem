// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for English (`en`).
class AppLocalizationsEn extends AppLocalizations {
  AppLocalizationsEn([String locale = 'en']) : super(locale);

  @override
  String get appTitle => 'MEM';

  @override
  String get conversations => 'Conversations';

  @override
  String get noContactsTitle => 'No contacts yet';

  @override
  String get noContactsBody =>
      'MEM only talks to people you have approved. Meet someone in person and scan each other\'s codes, or exchange codes through another app.';

  @override
  String get addContact => 'Add contact';

  @override
  String get receive => 'Receive';

  @override
  String get privacyLog => 'Privacy log';

  @override
  String get profile => 'Profile';

  @override
  String get pairTitle => 'Add contact';

  @override
  String get myCodeTab => 'My code';

  @override
  String get scanTab => 'Scan or paste';

  @override
  String get myCodeHint => 'Let your contact scan this code. Then scan theirs.';

  @override
  String get copyCode => 'Copy code';

  @override
  String get shareCode => 'Share code…';

  @override
  String get newCode => 'New pairing code';

  @override
  String get codeCopied => 'Code copied';

  @override
  String get shareCodeWarning =>
      'Sharing your code through another app tells that app that you use MEM and links your MEM identity to your account there. Meeting in person and scanning is more private.';

  @override
  String get continueAction => 'Continue';

  @override
  String get cancel => 'Cancel';

  @override
  String get scanHint => 'Point the camera at a MEM code';

  @override
  String get pasteHint => 'Paste a MEM code or message here';

  @override
  String get pasteFromClipboard => 'Paste from clipboard';

  @override
  String get open => 'Open';

  @override
  String get cameraUnavailable =>
      'Camera scanning is not available on this device. Paste the text instead.';

  @override
  String get contactAddedTitle => 'Contact added';

  @override
  String get contactUpdatedTitle => 'Contact re-paired';

  @override
  String get compareSas =>
      'Compare these symbols with the ones on your contact\'s screen. If they match, nobody is in the middle.';

  @override
  String get sasMatch => 'They match';

  @override
  String get sasMismatch => 'They don\'t match';

  @override
  String get sasMismatchBody =>
      'Ask your contact to show a new pairing code, then pair again. Do not send anything sensitive until the symbols match.';

  @override
  String get fingerprint => 'Fingerprint';

  @override
  String get metInPerson => 'Met in person';

  @override
  String get remote => 'Paired remotely';

  @override
  String get messageReceived => 'Message received';

  @override
  String get notMem => 'This text contains no MEM code or message.';

  @override
  String get typeMessage => 'Message';

  @override
  String get tapToSend => 'Not sent yet — tap to choose how';

  @override
  String get chooseChannel => 'How should this message travel?';

  @override
  String get needsConsent =>
      'Below your privacy floor — you\'ll see what is exposed';

  @override
  String get tooLarge => 'Too long for this channel';

  @override
  String get showQrTitle => 'Show this to your contact';

  @override
  String get showQrHint =>
      'Your contact opens MEM → Receive and points the camera at this screen.';

  @override
  String get done => 'Done';

  @override
  String get shareTitle => 'Send through another app';

  @override
  String get shareHint =>
      'Send this text to your contact through any app. They paste it into MEM → Receive.';

  @override
  String get copyText => 'Copy';

  @override
  String get shareText => 'Share…';

  @override
  String get copied => 'Copied';

  @override
  String get consentTitle => 'Privacy check';

  @override
  String whoCanSee(String observers) {
    return 'Who can see it: $observers';
  }

  @override
  String get theyLearn => 'They will learn';

  @override
  String get theyLearnPartly => 'They may partly learn';

  @override
  String get staysPrivate => 'Stays private';

  @override
  String get allowOnce => 'Allow for this message';

  @override
  String get allowContact => 'Allow for this contact…';

  @override
  String get dontSend => 'Don\'t send';

  @override
  String get confirmAlwaysTitle => 'Allow without asking?';

  @override
  String get confirmAlwaysBody =>
      'Future messages to this contact may use this channel without this dialog, for the time you choose. Every such send still appears in the Privacy log, and you can revoke it in the contact\'s settings.';

  @override
  String get allow => 'Allow';

  @override
  String viaChannel(String channel) {
    return 'via $channel';
  }

  @override
  String get channelQr => 'QR';

  @override
  String get channelShare => 'other app';

  @override
  String get contactSettings => 'Contact';

  @override
  String get privacyFloor => 'Privacy floor';

  @override
  String get privacyFloorHelp =>
      'Channels above this grade are used without asking. Lower grades always ask first and are recorded in the Privacy log.';

  @override
  String get grade0 => 'P0 · in person (QR, radio)';

  @override
  String get grade1 => 'P1 · direct network';

  @override
  String get grade2 => 'P2 · own relay or hub';

  @override
  String get grade3 => 'P3 · MEM couriers';

  @override
  String get grade4 => 'P4 · other apps (encrypted)';

  @override
  String get grade5 => 'P5 · plain text';

  @override
  String get standingRules => 'Standing permissions';

  @override
  String get noRules => 'None';

  @override
  String get revoke => 'Revoke';

  @override
  String get allContacts => 'all contacts';

  @override
  String get logEmpty => 'No message has gone below a privacy floor.';

  @override
  String get basisOnce => 'allowed once';

  @override
  String get basisNewRule => 'allowed, permission created';

  @override
  String get basisRule => 'allowed by permission';

  @override
  String logEntry(String grade, String floor, String basis) {
    return '$grade below floor $floor · $basis';
  }

  @override
  String get yourName => 'Your name (optional, shown on your code)';

  @override
  String get save => 'Save';

  @override
  String get saved => 'Saved';

  @override
  String get yourFingerprint => 'Your fingerprint';

  @override
  String get about => 'About this build';

  @override
  String get aboutBody =>
      'MEM v0.2 — experimental. Messages are end-to-end encrypted (HPKE, Ed25519). They travel by animated QR code in person, directly over the same Wi-Fi network, or as encrypted text through another app. Courier and hub delivery come later. Do not rely on it for high-risk communication until an external security review.';

  @override
  String error(String details) {
    return 'Something went wrong: $details';
  }

  @override
  String get reason_malformed => 'This is not valid MEM data.';

  @override
  String get reason_unsupported => 'Made by a newer MEM version.';

  @override
  String get reason_not_for_us =>
      'Not addressed to you, or from someone you haven\'t added.';

  @override
  String get reason_undecryptable =>
      'Could not be decrypted. Ask the sender to pair again.';

  @override
  String get reason_bad_signature => 'The sender could not be verified.';

  @override
  String get reason_duplicate => 'You already have this message.';

  @override
  String get reason_purged =>
      'You already received this message (or the sender withdrew it).';

  @override
  String get reason_expired => 'This message has expired.';

  @override
  String get reason_own_card => 'That\'s your own code.';

  @override
  String get reason_unknown_contact => 'Unknown contact.';

  @override
  String get reason_unknown_message => 'This message can no longer be sent.';

  @override
  String get reason_too_large_for_channel => 'Too long for this channel.';

  @override
  String get reason_consent_mismatch =>
      'Permission did not match. Please try again.';

  @override
  String get reason_invalid_text => 'The message is empty or too long.';

  @override
  String get reason_file_too_large =>
      'This file is too large. MEM sends files up to 100 MB, at most 16 at a time.';

  @override
  String get reason_invalid_attachment =>
      'This file cannot be attached: its name or type is too long.';

  @override
  String attachmentSending(int sent, int total) {
    return 'Sending $sent of $total';
  }

  @override
  String attachmentArriving(int have, int total) {
    return 'Arriving $have of $total';
  }

  @override
  String get attachmentMissing => 'This file is not on this device.';

  @override
  String get attachmentOpenElsewhere => 'Open in another app';

  @override
  String get attachmentOpenElsewhereBody =>
      'The app you choose gets this file, unencrypted, and may keep or upload it.';

  @override
  String get attachmentOpenElsewhereAction => 'Choose app';

  @override
  String get playerPlay => 'Play';

  @override
  String get playerPause => 'Pause';

  @override
  String get playerCannotPlay =>
      'This file cannot be played here. Open it in another app with the share button.';

  @override
  String get mediaPlayer => 'Built-in media player';

  @override
  String get mediaPlayerHelp =>
      'Video and audio files play inside MEM: they are decrypted into a private temporary file that is deleted when the player closes. Off, they open in another app, which then gets the file.';

  @override
  String get voiceCalls => 'Voice calls (test)';

  @override
  String get voiceCallsHelp =>
      'Call contacts over Wi-Fi Direct or the local network. Call setup travels end to end through MEM; the voice is encrypted. The contact learns your address on that network. Being tested: off by default.';

  @override
  String get callStart => 'Call';

  @override
  String get callCalling => 'Calling…';

  @override
  String get callIncoming => 'Incoming call';

  @override
  String get callConnecting => 'Connecting…';

  @override
  String get callAccept => 'Accept';

  @override
  String get callDecline => 'Decline';

  @override
  String get callMute => 'Mute';

  @override
  String get callUnmute => 'Unmute';

  @override
  String get callSpeaker => 'Speaker';

  @override
  String get callHangUp => 'Hang up';

  @override
  String get callEnded => 'Call ended';

  @override
  String get callDeclined => 'Declined';

  @override
  String get callBusy => 'Busy in another call';

  @override
  String get callFailed => 'The call could not connect';

  @override
  String get callUnanswered => 'No answer';

  @override
  String get callNoMicrophone =>
      'Calls need the microphone. Allow it in the phone\'s settings.';

  @override
  String get reason_invalid_name => 'The name is too long.';

  @override
  String get channelLan => 'Wi-Fi';

  @override
  String get nearby => 'nearby';

  @override
  String nearbyDevices(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count MEM devices nearby',
      one: '1 MEM device nearby',
      zero: 'No MEM devices nearby',
    );
    return '$_temp0';
  }

  @override
  String get lanOffShort => 'Wi-Fi delivery off';

  @override
  String get notNearby => 'Not nearby now';

  @override
  String get sendingNearby => 'Sending to a device nearby…';

  @override
  String get waitForWifi => 'Wait until nearby';

  @override
  String get waitForWifiHint =>
      'Sent automatically when you are on the same Wi-Fi, or next to each other with Bluetooth on';

  @override
  String get channels => 'Channels';

  @override
  String get channelsIntro =>
      'What others can learn when MEM uses each channel. Message contents are always end-to-end encrypted, except a P5 fallback you confirm per message.';

  @override
  String get lanToggle => 'Deliver over the local network';

  @override
  String get lanToggleHelp =>
      'While on, other devices on the same Wi-Fi can see that this device runs MEM and its network addresses. Turn off on networks you don\'t trust.';

  @override
  String scanProgress(int received, int needed) {
    return 'Receiving… $received of $needed';
  }

  @override
  String animatedQrHint(int needed) {
    return 'Keep this on screen until your contact\'s phone has it. Missed frames don\'t matter: any $needed are enough.';
  }

  @override
  String get reason_unreachable => 'Your contact is not nearby right now.';

  @override
  String get reason_channel_disabled =>
      'This channel is turned off in Channels.';

  @override
  String get reason_quota =>
      'Too much data from a nearby device; disconnected.';

  @override
  String get reason_unknown => 'Something unexpected happened.';

  @override
  String get appearance => 'Appearance';

  @override
  String get themeSystem => 'System';

  @override
  String get themeWhite => 'White';

  @override
  String get themeGray => 'Gray';

  @override
  String get themeBlack => 'Black';

  @override
  String get notConfirmed => 'Not confirmed yet — tap to send again';

  @override
  String get awaitingConfirmation => 'Waiting for confirmation';

  @override
  String get retryTitle => 'Retry unconfirmed messages';

  @override
  String retryDays(int days) {
    String _temp0 = intl.Intl.pluralLogic(
      days,
      locale: localeName,
      other: '$days days',
      one: '1 day',
    );
    return '$_temp0';
  }

  @override
  String get retryHelp =>
      'Messages your contact has not confirmed are sent again whenever you are connected, for up to this long. Applies to new messages.';

  @override
  String get backupTitle => 'Backup and recovery';

  @override
  String get backupIntro =>
      'A backup holds your identity, contacts, keys and, if you choose, your messages. It is encrypted: only your recovery phrase opens it. MEM keeps no copy anywhere.';

  @override
  String get backupOffBody =>
      'Turn on backups to get a recovery phrase. Write it on paper and keep it apart from your backup files: anyone who has both can read your messages and write as you.';

  @override
  String get phraseFormTitle => 'Recovery phrase';

  @override
  String get formEnglish => 'English words';

  @override
  String get formSyllables => 'Latin syllables';

  @override
  String get turnOnBackups => 'Turn on backups';

  @override
  String get phraseTitle => 'Your recovery phrase';

  @override
  String get phraseHint =>
      'Write these down in order, on paper. Screenshots are blocked on this screen.';

  @override
  String get phraseWritten => 'I have written it down';

  @override
  String get confirmTitle => 'Check your phrase';

  @override
  String get confirmHint => 'Type these words from your note.';

  @override
  String get confirmWrong => 'That does not match. Look at your note again.';

  @override
  String get confirmDone => 'Backups are on.';

  @override
  String wordN(int n) {
    return 'Word $n';
  }

  @override
  String get includeHistory => 'Include messages';

  @override
  String get saveBackupFile => 'Save backup file';

  @override
  String get backupSaved => 'Backup saved';

  @override
  String get showBackupQr => 'Show as QR code';

  @override
  String get backupQrHint =>
      'On the new phone, open MEM → Profile → Backup and recovery → Restore from backup, and point it at this screen.';

  @override
  String get backupTooLargeForQr =>
      'Too large for a QR code with messages: showing it without messages.';

  @override
  String get shareBackup => 'Send through another app…';

  @override
  String get shareBackupWarning =>
      'The app you choose will see that you have a MEM backup, roughly how large it is, and when you sent it. It cannot read it.';

  @override
  String get showPhrase => 'Show recovery phrase';

  @override
  String get newPhrase => 'New recovery phrase';

  @override
  String get newPhraseWarning =>
      'Backups you make from now on need the new phrase. Older backup files still open only with the old phrase.';

  @override
  String get scheduleTitle => 'Automatic backups';

  @override
  String get scheduleOff => 'Off';

  @override
  String scheduleEveryDays(int days) {
    String _temp0 = intl.Intl.pluralLogic(
      days,
      locale: localeName,
      other: 'Every $days days',
      one: 'Every day',
    );
    return '$_temp0';
  }

  @override
  String get scheduleAfterContact => 'After each new contact';

  @override
  String get backupFolder => 'Folder';

  @override
  String get chooseFolder => 'Choose';

  @override
  String get noFolder => 'No folder chosen';

  @override
  String get cloudFolderNote =>
      'If this folder syncs to a cloud service, that service sees each backup\'s size and time. It cannot read them.';

  @override
  String lastBackup(String when) {
    return 'Last backup: $when';
  }

  @override
  String get backupFailed => 'The last automatic backup failed.';

  @override
  String get restoreTitle => 'Restore from backup';

  @override
  String get restoreHint =>
      'Scan the QR code from your old phone, open a backup file, or paste backup text. Then type your recovery phrase.';

  @override
  String get openBackupFile => 'Open backup file';

  @override
  String get pasteBackup => 'Paste backup text';

  @override
  String get backupReceived => 'Backup received. Type your recovery phrase.';

  @override
  String get phraseInput => 'Recovery phrase';

  @override
  String get restoreAction => 'Restore';

  @override
  String get restoreReplaceTitle => 'Replace this identity?';

  @override
  String get restoreReplaceBody =>
      'This phone already has a MEM identity with contacts. Restoring replaces it. The current state is kept, encrypted, in the app\'s storage; it is not deleted.';

  @override
  String get restoreDone =>
      'Restored. If you still have your old phone, reset MEM there: until then, both can read new messages.';

  @override
  String restoreCorrected(String phrase) {
    return 'One letter in your phrase was corrected: $phrase';
  }

  @override
  String get restoreWrongPhrase => 'This phrase does not open this backup.';

  @override
  String get restoreNotBackup => 'This is not a MEM backup.';

  @override
  String get working => 'Working…';

  @override
  String get qrPaceEasy => 'Easy';

  @override
  String get qrPaceNormal => 'Normal';

  @override
  String get qrPaceFast => 'Fast';

  @override
  String get qrPaceHint =>
      'If the other phone can\'t read the code, choose Easy.';

  @override
  String get settings => 'Settings';

  @override
  String get language => 'Language';

  @override
  String get followSystem => 'System';

  @override
  String get textSize => 'Use the phone\'s text size';

  @override
  String percent(int value) {
    return '$value %';
  }

  @override
  String get sectionMessages => 'Messages';

  @override
  String get showMessageTimes => 'Show message times';

  @override
  String get badgeDetail => 'Channel badges';

  @override
  String get badgeCompact => 'Compact';

  @override
  String get badgeDetailed => 'Detailed';

  @override
  String get badgeDetailHelp =>
      'Detailed full badges also show when each channel carried the message, and whether it was delivered.';

  @override
  String get messageBadges => 'Message badges';

  @override
  String get messageBadgesHelp =>
      'Channel badges sit at the bottom left of each message, on the line of its time. They can shrink to a symbol after a while; tap the badge area to see them in full. Other apps (P4), plain text (P5) and channels below the contact\'s privacy floor always keep their symbol.';

  @override
  String get badgeModeMinimal => 'Symbols only for P4, P5 and below the floor';

  @override
  String get badgeModeSymbols => 'Symbols only';

  @override
  String badgeModeCollapse(String delay) {
    return 'Full, then symbols after $delay';
  }

  @override
  String get badgeModeFull => 'Always full';

  @override
  String durationSeconds(int n) {
    return '$n s';
  }

  @override
  String durationMinutes(int n) {
    return '$n min';
  }

  @override
  String durationHours(int n) {
    return '$n h';
  }

  @override
  String get badgeSymbols => 'Badge symbols';

  @override
  String get badgeSymbolsByGrade => 'By grade';

  @override
  String get badgeSymbolsOneShape => 'One shape';

  @override
  String get badgeSymbolsHelp =>
      'By grade gives each privacy grade its own shape. One shape uses the shape you pick for P0 to P3; P4 and P5 always keep their own.';

  @override
  String get sectionQr => 'QR codes';

  @override
  String get qrPaceTitle => 'Animated QR speed';

  @override
  String get sectionPrivacy => 'Privacy';

  @override
  String get defaultFloor => 'Privacy floor for new contacts';

  @override
  String get defaultFloorHelp =>
      'Channels below the floor need your permission before a message goes out. Contacts you already have keep their own floor; change it on the contact\'s page.';

  @override
  String get blockScreenshots => 'Block screenshots';

  @override
  String get blockScreenshotsHelp =>
      'No screenshots or screen recording, and MEM\'s content is hidden in the recent apps list. The recovery phrase is always protected.';

  @override
  String get showNetworkStatus => 'Show network status on the main screen';

  @override
  String get statusDelivered => 'Delivered';

  @override
  String get statusReceived => 'Received';

  @override
  String get reason_invalid_floor =>
      'P5 cannot be a privacy floor: plaintext always needs your permission for each message.';

  @override
  String get phraseNotConfirmed =>
      'The new phrase was not confirmed, so nothing changed.';

  @override
  String get scanFocusHint => 'Blurry? Tap the picture to focus.';

  @override
  String get helpTooltip => 'How it works';

  @override
  String get scanBrightness => 'Brightness of the camera picture';

  @override
  String get codeBrightness => 'Screen brightness for the code';

  @override
  String get stayReachable => 'Stay reachable in the background';

  @override
  String get stayReachableHelp =>
      'Keeps Wi-Fi and Bluetooth delivery working with the screen off. Shows a permanent notification, so anyone who sees your phone can tell MEM is running, and uses more battery.';

  @override
  String get reachableNotification => 'Reachable over Wi-Fi';

  @override
  String get batteryRestricted =>
      'Android may still pause MEM to save battery. Tap to let it run unrestricted.';

  @override
  String get batteryUnrestricted => 'MEM may run unrestricted.';

  @override
  String get sectionCourier => 'Courier';

  @override
  String get courierMode => 'Carry messages for other MEM users';

  @override
  String get courierModeHelp =>
      'Your phone keeps sealed messages for people nearby and passes them on when it meets their phones. It cannot read them: it sees only scrambled addresses, sizes and times. Uses storage and some battery.';

  @override
  String courierLimit(int mb) {
    return 'Up to $mb MB';
  }

  @override
  String courierUsage(int count, String used, String limit) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Carrying $count messages',
      one: 'Carrying 1 message',
    );
    return '$_temp0 · $used of $limit MB';
  }

  @override
  String get channelCourier => 'couriers';

  @override
  String get courierQueued =>
      'It will travel with the next MEM device you meet.';

  @override
  String get courierHint => 'Goes with nearby MEM devices that cannot read it.';

  @override
  String get reason_waiting_for_key =>
      'Waiting for your contact\'s new encryption key. Any message from them brings it, and this message then goes out by itself.';

  @override
  String get requestTab => 'Request';

  @override
  String get requestHint =>
      'For someone you cannot meet: paste their MEM code. They get a contact request with your code and your note, and decide whether to accept. It travels through nearby MEM devices, so it can take a while.';

  @override
  String get requestCodeLabel => 'Their code (memcard1:…)';

  @override
  String get requestNoteLabel => 'Note (optional)';

  @override
  String get requestNoteHelp =>
      'Shown with your request: who you are and how they know you.';

  @override
  String get sendRequest => 'Send request';

  @override
  String get requestPreparing =>
      'Preparing the request. This takes a few seconds.';

  @override
  String requestPendingBanner(String name) {
    return 'Waiting for $name to accept your contact request. You can write once they do.';
  }

  @override
  String get contactRequestLabel => 'Contact request';

  @override
  String get requestNoNote => '(no note)';

  @override
  String get requestsTitle => 'Contact requests';

  @override
  String requestsTooltip(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count contact requests',
      one: '1 contact request',
    );
    return '$_temp0';
  }

  @override
  String get requestsEmpty => 'No contact requests.';

  @override
  String get requestsHelp =>
      'These people have your MEM code and want to become contacts. Names are not verified: compare the safety symbols when you meet.';

  @override
  String nameNotVerified(String name) {
    return '$name (name not verified)';
  }

  @override
  String get acceptRequest => 'Accept';

  @override
  String get declineRequest => 'Decline';

  @override
  String get blockRequest => 'Block';

  @override
  String get blockConfirm =>
      'Block this sender? Their contact requests will be ignored from now on.';

  @override
  String get requestAccepted => 'Contact added';

  @override
  String get requestReceived => 'Contact request received';

  @override
  String get requestsNearby =>
      'Accept contact requests from people you haven\'t met';

  @override
  String get requestsNearbyHelp =>
      'Requests come through nearby MEM devices and, if you use one, the hub. Anyone who has your MEM code could notice when your phone is nearby, and the hub could link your internet address to your code. Requests you paste or scan arrive either way.';

  @override
  String get requestedSubtitle => 'Request sent · waiting for an answer';

  @override
  String get reason_awaiting_approval =>
      'They have not accepted your contact request yet.';

  @override
  String get reason_bad_proof_of_work => 'This contact request is not valid.';

  @override
  String get reason_unknown_request => 'This request was already answered.';

  @override
  String get reason_already_contact =>
      'You are already contacts, or a request is on its way.';

  @override
  String get reason_invalid_note => 'The note is too long (at most 280 bytes).';

  @override
  String get reason_not_a_card => 'That is not a MEM code.';

  @override
  String get channelBle => 'Bluetooth';

  @override
  String get bleToggle => 'Deliver over Bluetooth';

  @override
  String get bleToggleHelp =>
      'Phones next to each other exchange messages without any network. While on, any Bluetooth scanner nearby can tell that a MEM device is here, though not whose.';

  @override
  String get bleLocationExplain =>
      'On this Android version, Android lets apps look for Bluetooth devices only with the location permission and with Location switched on. MEM never reads your location.';

  @override
  String get bleStatusRunning => 'Looking for MEM devices nearby';

  @override
  String get bleStatusOff => 'Bluetooth is switched off';

  @override
  String get bleStatusPermission => 'Bluetooth permission is missing';

  @override
  String get bleStatusLocation =>
      'Location is off, so Android hides nearby devices';

  @override
  String get bleStatusUnsupported => 'This phone has no Bluetooth LE';

  @override
  String get bleTurnOn => 'Turn on';

  @override
  String get bleOpenSettings => 'Open settings';

  @override
  String get bleBackgroundNote =>
      'In the background, only while \"Stay reachable in the background\" is on.';

  @override
  String get reachableNotificationBle => 'Reachable over Wi-Fi and Bluetooth';

  @override
  String get newGroup => 'New group';

  @override
  String get groupName => 'Group name';

  @override
  String get groupMembersPick => 'Who to invite (up to 15 contacts)';

  @override
  String get groupCreate => 'Create';

  @override
  String get groupCreateHelp =>
      'You will be the group\'s admin: only you add or remove members. Everyone you invite sees the names and codes of the members who join.';

  @override
  String groupMembersCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count members',
      one: '1 member',
    );
    return '$_temp0';
  }

  @override
  String get groupJoining => 'Joining… waiting for the admin';

  @override
  String get groupInvitedBanner => 'You are invited to this group.';

  @override
  String get groupLeftBanner => 'You left this group.';

  @override
  String get groupRemovedBanner => 'The admin removed you from this group.';

  @override
  String get groupJoiningBanner =>
      'Waiting for the admin to add you. Messages can be sent once you are in.';

  @override
  String get groupJoin => 'Join';

  @override
  String get groupDecline => 'Decline';

  @override
  String get groupInvitationsTitle => 'Group invitations';

  @override
  String groupInvitedBy(String name) {
    return 'Invited by $name';
  }

  @override
  String get groupInvitationHelp =>
      'If you join, every member sees your name and MEM code, and you see theirs.';

  @override
  String get groupInfo => 'Group info';

  @override
  String get groupAdmin => 'admin';

  @override
  String get groupInvitedRole => 'invited';

  @override
  String get groupYou => 'you';

  @override
  String get groupNotAContact => 'not your contact';

  @override
  String get groupAddAsContact => 'Add to contacts';

  @override
  String groupAddAsContactHelp(String name) {
    return '$name becomes your contact. You can write to each other directly once they add you too.';
  }

  @override
  String get groupInvite => 'Invite a contact';

  @override
  String get groupRemove => 'Remove from group';

  @override
  String get groupLeave => 'Leave group';

  @override
  String get groupEnd => 'End group for everyone';

  @override
  String get groupLeaveConfirm =>
      'Leave this group? You will not receive its messages any more.';

  @override
  String get groupEndConfirm => 'End this group? Every member is removed.';

  @override
  String deliveredToCount(int done, int total) {
    return 'delivered to $done of $total';
  }

  @override
  String get groupSentAutomatically =>
      'Sent to each member automatically when a way to them opens';

  @override
  String get groupsSection => 'Groups';

  @override
  String get contactsSection => 'Contacts';

  @override
  String get reason_unknown_group => 'That group is not available.';

  @override
  String get reason_not_group_admin => 'Only the group\'s admin can do that.';

  @override
  String get reason_group_full =>
      'A group has at most 16 members (and you can keep at most 64 groups).';

  @override
  String get reason_group_empty => 'Nobody else is in this group yet.';

  @override
  String get reason_not_a_contact =>
      'This person is only a fellow group member, not your contact.';

  @override
  String get channelHub => 'Hub';

  @override
  String get hubToggle => 'Deliver through a MEM hub';

  @override
  String get hubToggleHelp =>
      'Messages wait in a mailbox on the Internet until your contact collects them, so phones far apart still exchange messages. The hub\'s operator sees your IP address, when you connect, message sizes and short parts of your mailbox addresses, never the content. Anyone watching your network can tell you use MEM.';

  @override
  String get hubUrl => 'Hub address';

  @override
  String get hubUrlHint => 'wss://hub.example.org/v1';

  @override
  String get hubUrlSave => 'Save';

  @override
  String get hubNeedsUrl =>
      'Enter the address of a hub first. There is no default hub yet.';

  @override
  String get hubOnline => 'Connected to the hub';

  @override
  String get hubOffline => 'Not connected; retrying';

  @override
  String get devices => 'Devices';

  @override
  String get devicesHelp =>
      'Your MEM can be on up to four devices. The main device holds your identity: only it adds contacts and devices. People who carry your mail cannot tell the devices apart, but may notice that one message goes out several times at once.';

  @override
  String get thisDevice => 'This device';

  @override
  String get mainDevice => 'Main device';

  @override
  String get otherDevice => 'Other device';

  @override
  String get deviceWaiting =>
      'Not joined yet. If adding it didn\'t finish, remove it and add the device again.';

  @override
  String get addDevice => 'Add a device';

  @override
  String get addDeviceHelp =>
      'On the new device, choose “Use as another device” on the start screen. Then scan the code it shows.';

  @override
  String get addDeviceConfirmTitle => 'Add this device?';

  @override
  String get addDeviceConfirmBody =>
      'It will get all your contacts and receive your messages. Add only a device of your own, here with you.';

  @override
  String get addDeviceShowSetup => 'Show this to the new device';

  @override
  String get addDeviceSetupHint =>
      'Hold the new device\'s camera to this code until it says it is ready.';

  @override
  String get removeDevice => 'Remove';

  @override
  String get removeDeviceTitle => 'Remove this device?';

  @override
  String get removeDeviceBody =>
      'It stops receiving your messages, and deletes its copy of your MEM the next time it is online.';

  @override
  String get removeWaitingDeviceBody =>
      'It never joined, so it has none of your messages. If it still reads the setup code, it will learn that it was removed.';

  @override
  String get joinAsDevice => 'Use as another device';

  @override
  String get joinTitle => 'Another device of my MEM';

  @override
  String get joinHelp =>
      'On your main device, open ⋮ → Devices → Add a device and scan this code. Then point this camera at the code your main device shows.';

  @override
  String get joinDone => 'This device is now part of your MEM.';

  @override
  String get deviceRetiredTitle => 'This device was removed';

  @override
  String get deviceRetiredBody =>
      'Your main device removed it from your MEM. Its copy will be deleted now.';

  @override
  String get addContactOnMain =>
      'New contacts are added on your main device; they appear here by themselves.';

  @override
  String get reason_main_device_only => 'Only your main device can do this.';

  @override
  String get reason_too_many_devices => 'Your MEM already has four devices.';

  @override
  String get reason_device_known => 'That device is already one of yours.';

  @override
  String get reason_unknown_device => 'That is not one of your devices.';

  @override
  String get reason_not_empty =>
      'This device already has contacts. Only a new installation can become another device.';

  @override
  String get reason_not_for_this_device => 'That code is for another device.';

  @override
  String get joinWrongSetup =>
      'This code was made for an earlier request, so it can\'t be opened here. On the main device, remove the device marked “Not joined yet”, then add this device again by scanning the code above.';

  @override
  String get channelHubPartners => 'Hubs of several operators';

  @override
  String hubPartners(String names) {
    return 'This hub also exchanges messages with the hubs of: $names. A message to a contact who uses one of them is carried by both operators; both see what a hub sees. Such messages are marked P3.';
  }

  @override
  String get ownOperatorOnly => 'Only my operator\'s hubs';

  @override
  String get ownOperatorOnlyHelp =>
      'Your messages never go to other operators\' hubs. Contacts who use another operator\'s hub then cannot get them through the hub.';

  @override
  String get reason_invalid_hub_url =>
      'Use a wss:// address (ws:// only for a hub on this device).';

  @override
  String get pushNotification => 'New messages';

  @override
  String get pushToggle => 'Wake up for new messages';

  @override
  String get pushToggleHelp =>
      'A UnifiedPush app on this phone (for example ntfy) wakes MEM when the hub has something for you, so messages arrive with MEM closed. The push service learns when MEM is woken, nothing else. Needs the hub.';

  @override
  String get pushNoDistributor =>
      'Install a UnifiedPush app (for example ntfy) first.';

  @override
  String get pushPick => 'Which push app?';

  @override
  String get channelPlain => 'Plain text';

  @override
  String get plainTitle => 'Send as plain text?';

  @override
  String get plainWarning =>
      'This exact text leaves MEM unencrypted. The app you pick, its operator, and anyone who can read that chat will see it. It cannot be taken back.';

  @override
  String get plainSend => 'Send as plain text';

  @override
  String get reason_not_plain_text =>
      'Only a one-to-one text message can be sent as plain text.';

  @override
  String get channelEmail => 'Email';

  @override
  String get emailToggle => 'Deliver by email';

  @override
  String get emailToggleHelp =>
      'MEM sends sealed messages from your mail account and collects MEM mails from it. Your mail provider sees both addresses, when, how much, and that it is MEM mail, never the content. Below your default privacy floor, so each send asks unless you allow email for a contact.';

  @override
  String get emailAddress => 'Your email address';

  @override
  String get emailImapHost => 'IMAP server';

  @override
  String get emailSmtpHost => 'SMTP server';

  @override
  String get emailPort => 'Port';

  @override
  String get emailUsername => 'Login';

  @override
  String get emailPassword => 'App password';

  @override
  String get emailPasswordKept => 'App password (saved; leave empty to keep)';

  @override
  String get emailSaveAccount => 'Save account';

  @override
  String get emailRemoveAccount => 'Remove account';

  @override
  String get emailOnline => 'Connected to your mailbox';

  @override
  String get emailOffline => 'Not connected; retrying';

  @override
  String get emailSaved => 'Mail account saved';

  @override
  String get contactEmail => 'Email address';

  @override
  String get contactEmailHelp =>
      'Only on this phone; MEM mails sealed messages here when you allow email for this contact.';

  @override
  String get contactEmailSave => 'Save';

  @override
  String get contactAliasEdit => 'Rename';

  @override
  String get contactAliasTitle => 'Name for this contact';

  @override
  String get contactAliasHelp =>
      'Only you see this name; your contact is not told. Leave it empty to show the name they chose.';

  @override
  String contactTheirName(String name) {
    return 'Their own name: $name';
  }

  @override
  String get contactTheirNameNone => 'They chose no name';

  @override
  String get reason_invalid_email_account =>
      'Check the mail account: address, servers, ports, login and app password are all needed.';

  @override
  String get reason_invalid_email_address => 'That is not an email address.';

  @override
  String get alreadyContact => 'Already in your contacts';

  @override
  String get compareSymbols => 'Compare symbols';

  @override
  String get channelWifiDirect => 'Wi-Fi Direct';

  @override
  String get wifiDirectToggle => 'Deliver over Wi-Fi Direct';

  @override
  String get wifiDirectToggleHelp =>
      'Phones nearby exchange messages over Wi-Fi without a router or the Internet. While on, nearby phones can see that yours runs MEM, and Android shows your phone\'s Wi-Fi Direct name to them. On Android 9 and older the other phone is asked to accept each connection. Android 12 and older need Location on to find phones; MEM never reads your location.';

  @override
  String get wifiDirectUnsupported => 'This phone has no Wi-Fi Direct.';

  @override
  String get inboundCardTitle => 'Add this contact?';

  @override
  String get inboundCardBody =>
      'Another app handed MEM a contact code. Add it only if you meant to: this person will be able to message you. Check the fingerprint with them if you can.';

  @override
  String get wifiDirectPermission =>
      'Permission to find nearby phones is missing';

  @override
  String get wifiDirectLocation =>
      'Location is off: this Android version needs it to find phones over Wi-Fi Direct';

  @override
  String get removeContact => 'Remove contact';

  @override
  String get removeContactTitle => 'Remove this contact?';

  @override
  String get removeContactBody =>
      'Your conversation and any messages still waiting to be sent are deleted, and their messages will no longer reach you. Groups you share keep working. To talk again you need to pair again.';

  @override
  String get ruleHowLong => 'For how long?';

  @override
  String get ruleDays1 => '1 day';

  @override
  String get ruleDays7 => '7 days';

  @override
  String get ruleDays30 => '30 days';

  @override
  String get ruleUntilRevoked => 'Until I revoke';

  @override
  String ruleUntil(String date) {
    return 'until $date';
  }

  @override
  String get soundPairButton => 'Pair by sound';

  @override
  String get soundPairTitle => 'Pair by sound';

  @override
  String get soundPairHint =>
      'Hold the phones close. On one phone tap Listen, on the other Play my code. The phones then take turns by themselves; at the end both show the same symbols. Compare them and tap Connect on both phones.';

  @override
  String soundPairProgress(int received, int needed) {
    return 'Heard $received of $needed parts';
  }

  @override
  String get soundListen => 'Listen';

  @override
  String get soundPlay => 'Play my code';

  @override
  String get soundStop => 'Stop';

  @override
  String get soundIdle =>
      'Choose what this phone does: listen, or play its code.';

  @override
  String get soundListening => 'Listening for the other phone…';

  @override
  String soundPlaying(int round, int rounds) {
    return 'Playing my code, round $round of $rounds…';
  }

  @override
  String get soundListenTimeout =>
      'Nothing heard for two minutes. Bring the phones closer or make the room quieter, then listen again.';

  @override
  String get soundPlayDone =>
      'My code was played three times. If the other phone did not get it, play it again.';

  @override
  String get soundTurnWait =>
      'Their code arrived. Waiting for a quiet moment to play mine…';

  @override
  String get soundNoAnswer =>
      'The other phone did not answer. Bring the phones closer or make the room quieter, then start again.';

  @override
  String get soundDecline => 'Decline';

  @override
  String get soundConnect => 'Connect';

  @override
  String get soundPaired => 'Connected. You can write to each other now.';

  @override
  String get soundDeclined =>
      'Declined. A contact this exchange added was removed.';

  @override
  String get soundDeclinedKept =>
      'Declined. The contact you already had stays as it was.';

  @override
  String get soundPairGot => 'Their code arrived.';

  @override
  String get soundPairNoMicrophone =>
      'Pairing by sound needs the microphone. Allow it in the phone\'s settings, or pair with the QR code.';

  @override
  String get soundPairFailed =>
      'Sound could not start on this phone. Pair with the QR code instead.';

  @override
  String get soundToneTitle => 'Sound';

  @override
  String get soundToneAudible => 'Audible';

  @override
  String get soundToneInaudible => 'Almost inaudible';

  @override
  String get soundPairExposure =>
      'Anyone within earshot, or any recording, can pick up your code, just as anyone who sees your QR code can. Messages never travel by sound. Both phones must use the same sound.';

  @override
  String get channelAudio => 'Sound';

  @override
  String get nearbyTitle => 'Nearby';

  @override
  String get connectBySound => 'Connect by sound';

  @override
  String get connectByCode => 'Connect by code';

  @override
  String get nearbyHelp =>
      'Phones on the same Wi-Fi usually find each other by themselves. Some networks (guest, hotel, office) block that: then connect by sound or by code. One phone tells the other where to reach it on this Wi-Fi; the messages themselves go over Wi-Fi.';

  @override
  String get connectSoundHint =>
      'Hold the phones close. On one phone tap Listen, on the other Play my code. The listening phone connects to the playing one over this Wi-Fi; one code takes about 6 seconds.';

  @override
  String get connectSoundExposure =>
      'People nearby, or a recording, can hear that a MEM phone is here, its address on this network and a temporary key that changes when MEM restarts. Nothing that identifies you, and no messages.';

  @override
  String get connectGot =>
      'Connecting to the other phone. Messages for your contacts follow by themselves.';

  @override
  String get connectNoMicrophone =>
      'Connecting by sound needs the microphone. Allow it in the phone\'s settings, or connect by code.';

  @override
  String get connectFailed =>
      'Sound could not start on this phone. Connect by code instead.';

  @override
  String get connectCodeHint =>
      'Let the other phone scan this code, or scan theirs below.';

  @override
  String get reason_no_local_network =>
      'Turn on Wi-Fi and the local network channel (Channels) first.';

  @override
  String get attestTitle => 'Confirm in person';

  @override
  String get attestScanRequest =>
      'Scan the code on the service\'s page, or paste it. You confirm together with the other person, each on your own phone.';

  @override
  String get attestMainOnly => 'Only your main device can confirm in person.';

  @override
  String get attestAskedBy => 'Asked by';

  @override
  String get attestWhat => 'What';

  @override
  String get attestRole => 'Your role';

  @override
  String attestCheckCode(String code) {
    return 'Check that the page shows the same code: $code';
  }

  @override
  String get attestExchangeHint =>
      'Show your code to the other phone, then scan theirs.';

  @override
  String get attestCompare =>
      'Compare these symbols with the ones on the other phone. Confirm only if they match and the other person is in front of you.';

  @override
  String get attestSending => 'Sending…';

  @override
  String attestSent(String origin) {
    return 'Sent. $origin now has your confirmation.';
  }

  @override
  String get attestSendFailed =>
      'It could not be sent. Copy this code and paste it on the service\'s page.';

  @override
  String get attestTryAgain => 'Try again';

  @override
  String get attestCancelled => 'Cancelled. Nothing was signed.';

  @override
  String get attestErrWrongObject => 'This is not the code needed here.';

  @override
  String get attestErrInvalid => 'This code is damaged or not allowed.';

  @override
  String get attestErrOtherSubject =>
      'The other phone is confirming something else.';

  @override
  String get attestErrSameRole =>
      'Both phones have the same role. Each person must scan their own code on the service\'s page.';

  @override
  String get attestErrOwnCode => 'This is your own code. Scan the other phone.';

  @override
  String get attestLog => 'Confirmed in person';

  @override
  String attestLogEntry(String origin, String how) {
    return '$origin · $how';
  }

  @override
  String get attestLogSent => 'sent';

  @override
  String get attestLogCopied => 'copied';
}
