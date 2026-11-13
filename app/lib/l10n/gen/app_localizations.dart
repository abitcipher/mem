import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:intl/intl.dart' as intl;

import 'app_localizations_en.dart';
import 'app_localizations_uk.dart';

// ignore_for_file: type=lint

/// Callers can lookup localized strings with an instance of AppLocalizations
/// returned by `AppLocalizations.of(context)`.
///
/// Applications need to include `AppLocalizations.delegate()` in their app's
/// `localizationDelegates` list, and the locales they support in the app's
/// `supportedLocales` list. For example:
///
/// ```dart
/// import 'gen/app_localizations.dart';
///
/// return MaterialApp(
///   localizationsDelegates: AppLocalizations.localizationsDelegates,
///   supportedLocales: AppLocalizations.supportedLocales,
///   home: MyApplicationHome(),
/// );
/// ```
///
/// ## Update pubspec.yaml
///
/// Please make sure to update your pubspec.yaml to include the following
/// packages:
///
/// ```yaml
/// dependencies:
///   # Internationalization support.
///   flutter_localizations:
///     sdk: flutter
///   intl: any # Use the pinned version from flutter_localizations
///
///   # Rest of dependencies
/// ```
///
/// ## iOS Applications
///
/// iOS applications define key application metadata, including supported
/// locales, in an Info.plist file that is built into the application bundle.
/// To configure the locales supported by your app, you’ll need to edit this
/// file.
///
/// First, open your project’s ios/Runner.xcworkspace Xcode workspace file.
/// Then, in the Project Navigator, open the Info.plist file under the Runner
/// project’s Runner folder.
///
/// Next, select the Information Property List item, select Add Item from the
/// Editor menu, then select Localizations from the pop-up menu.
///
/// Select and expand the newly-created Localizations item then, for each
/// locale your application supports, add a new item and select the locale
/// you wish to add from the pop-up menu in the Value field. This list should
/// be consistent with the languages listed in the AppLocalizations.supportedLocales
/// property.
abstract class AppLocalizations {
  AppLocalizations(String locale)
    : localeName = intl.Intl.canonicalizedLocale(locale.toString());

  final String localeName;

  static AppLocalizations of(BuildContext context) {
    return Localizations.of<AppLocalizations>(context, AppLocalizations)!;
  }

  static const LocalizationsDelegate<AppLocalizations> delegate =
      _AppLocalizationsDelegate();

  /// A list of this localizations delegate along with the default localizations
  /// delegates.
  ///
  /// Returns a list of localizations delegates containing this delegate along with
  /// GlobalMaterialLocalizations.delegate, GlobalCupertinoLocalizations.delegate,
  /// and GlobalWidgetsLocalizations.delegate.
  ///
  /// Additional delegates can be added by appending to this list in
  /// MaterialApp. This list does not have to be used at all if a custom list
  /// of delegates is preferred or required.
  static const List<LocalizationsDelegate<dynamic>> localizationsDelegates =
      <LocalizationsDelegate<dynamic>>[
        delegate,
        GlobalMaterialLocalizations.delegate,
        GlobalCupertinoLocalizations.delegate,
        GlobalWidgetsLocalizations.delegate,
      ];

  /// A list of this localizations delegate's supported locales.
  static const List<Locale> supportedLocales = <Locale>[
    Locale('en'),
    Locale('uk'),
  ];

  /// No description provided for @appTitle.
  ///
  /// In en, this message translates to:
  /// **'MEM'**
  String get appTitle;

  /// No description provided for @conversations.
  ///
  /// In en, this message translates to:
  /// **'Conversations'**
  String get conversations;

  /// No description provided for @noContactsTitle.
  ///
  /// In en, this message translates to:
  /// **'No contacts yet'**
  String get noContactsTitle;

  /// No description provided for @noContactsBody.
  ///
  /// In en, this message translates to:
  /// **'MEM only talks to people you have approved. Meet someone in person and scan each other\'s codes, or exchange codes through another app.'**
  String get noContactsBody;

  /// No description provided for @addContact.
  ///
  /// In en, this message translates to:
  /// **'Add contact'**
  String get addContact;

  /// No description provided for @receive.
  ///
  /// In en, this message translates to:
  /// **'Receive'**
  String get receive;

  /// No description provided for @privacyLog.
  ///
  /// In en, this message translates to:
  /// **'Privacy log'**
  String get privacyLog;

  /// No description provided for @profile.
  ///
  /// In en, this message translates to:
  /// **'Profile'**
  String get profile;

  /// No description provided for @pairTitle.
  ///
  /// In en, this message translates to:
  /// **'Add contact'**
  String get pairTitle;

  /// No description provided for @myCodeTab.
  ///
  /// In en, this message translates to:
  /// **'My code'**
  String get myCodeTab;

  /// No description provided for @scanTab.
  ///
  /// In en, this message translates to:
  /// **'Scan or paste'**
  String get scanTab;

  /// No description provided for @myCodeHint.
  ///
  /// In en, this message translates to:
  /// **'Let your contact scan this code. Then scan theirs.'**
  String get myCodeHint;

  /// No description provided for @copyCode.
  ///
  /// In en, this message translates to:
  /// **'Copy code'**
  String get copyCode;

  /// No description provided for @shareCode.
  ///
  /// In en, this message translates to:
  /// **'Share code…'**
  String get shareCode;

  /// No description provided for @newCode.
  ///
  /// In en, this message translates to:
  /// **'New pairing code'**
  String get newCode;

  /// No description provided for @codeCopied.
  ///
  /// In en, this message translates to:
  /// **'Code copied'**
  String get codeCopied;

  /// No description provided for @shareCodeWarning.
  ///
  /// In en, this message translates to:
  /// **'Sharing your code through another app tells that app that you use MEM and links your MEM identity to your account there. Meeting in person and scanning is more private.'**
  String get shareCodeWarning;

  /// No description provided for @continueAction.
  ///
  /// In en, this message translates to:
  /// **'Continue'**
  String get continueAction;

  /// No description provided for @cancel.
  ///
  /// In en, this message translates to:
  /// **'Cancel'**
  String get cancel;

  /// No description provided for @scanHint.
  ///
  /// In en, this message translates to:
  /// **'Point the camera at a MEM code'**
  String get scanHint;

  /// No description provided for @pasteHint.
  ///
  /// In en, this message translates to:
  /// **'Paste a MEM code or message here'**
  String get pasteHint;

  /// No description provided for @pasteFromClipboard.
  ///
  /// In en, this message translates to:
  /// **'Paste from clipboard'**
  String get pasteFromClipboard;

  /// No description provided for @open.
  ///
  /// In en, this message translates to:
  /// **'Open'**
  String get open;

  /// No description provided for @cameraUnavailable.
  ///
  /// In en, this message translates to:
  /// **'Camera scanning is not available on this device. Paste the text instead.'**
  String get cameraUnavailable;

  /// No description provided for @contactAddedTitle.
  ///
  /// In en, this message translates to:
  /// **'Contact added'**
  String get contactAddedTitle;

  /// No description provided for @contactUpdatedTitle.
  ///
  /// In en, this message translates to:
  /// **'Contact re-paired'**
  String get contactUpdatedTitle;

  /// No description provided for @compareSas.
  ///
  /// In en, this message translates to:
  /// **'Compare these symbols with the ones on your contact\'s screen. If they match, nobody is in the middle.'**
  String get compareSas;

  /// No description provided for @sasMatch.
  ///
  /// In en, this message translates to:
  /// **'They match'**
  String get sasMatch;

  /// No description provided for @sasMismatch.
  ///
  /// In en, this message translates to:
  /// **'They don\'t match'**
  String get sasMismatch;

  /// No description provided for @sasMismatchBody.
  ///
  /// In en, this message translates to:
  /// **'Ask your contact to show a new pairing code, then pair again. Do not send anything sensitive until the symbols match.'**
  String get sasMismatchBody;

  /// No description provided for @fingerprint.
  ///
  /// In en, this message translates to:
  /// **'Fingerprint'**
  String get fingerprint;

  /// No description provided for @metInPerson.
  ///
  /// In en, this message translates to:
  /// **'Met in person'**
  String get metInPerson;

  /// No description provided for @remote.
  ///
  /// In en, this message translates to:
  /// **'Paired remotely'**
  String get remote;

  /// No description provided for @messageReceived.
  ///
  /// In en, this message translates to:
  /// **'Message received'**
  String get messageReceived;

  /// No description provided for @notMem.
  ///
  /// In en, this message translates to:
  /// **'This text contains no MEM code or message.'**
  String get notMem;

  /// No description provided for @typeMessage.
  ///
  /// In en, this message translates to:
  /// **'Message'**
  String get typeMessage;

  /// No description provided for @tapToSend.
  ///
  /// In en, this message translates to:
  /// **'Not sent yet — tap to choose how'**
  String get tapToSend;

  /// No description provided for @chooseChannel.
  ///
  /// In en, this message translates to:
  /// **'How should this message travel?'**
  String get chooseChannel;

  /// No description provided for @needsConsent.
  ///
  /// In en, this message translates to:
  /// **'Below your privacy floor — you\'ll see what is exposed'**
  String get needsConsent;

  /// No description provided for @tooLarge.
  ///
  /// In en, this message translates to:
  /// **'Too long for this channel'**
  String get tooLarge;

  /// No description provided for @showQrTitle.
  ///
  /// In en, this message translates to:
  /// **'Show this to your contact'**
  String get showQrTitle;

  /// No description provided for @showQrHint.
  ///
  /// In en, this message translates to:
  /// **'Your contact opens MEM → Receive and points the camera at this screen.'**
  String get showQrHint;

  /// No description provided for @done.
  ///
  /// In en, this message translates to:
  /// **'Done'**
  String get done;

  /// No description provided for @shareTitle.
  ///
  /// In en, this message translates to:
  /// **'Send through another app'**
  String get shareTitle;

  /// No description provided for @shareHint.
  ///
  /// In en, this message translates to:
  /// **'Send this text to your contact through any app. They paste it into MEM → Receive.'**
  String get shareHint;

  /// No description provided for @copyText.
  ///
  /// In en, this message translates to:
  /// **'Copy'**
  String get copyText;

  /// No description provided for @shareText.
  ///
  /// In en, this message translates to:
  /// **'Share…'**
  String get shareText;

  /// No description provided for @copied.
  ///
  /// In en, this message translates to:
  /// **'Copied'**
  String get copied;

  /// No description provided for @consentTitle.
  ///
  /// In en, this message translates to:
  /// **'Privacy check'**
  String get consentTitle;

  /// No description provided for @whoCanSee.
  ///
  /// In en, this message translates to:
  /// **'Who can see it: {observers}'**
  String whoCanSee(String observers);

  /// No description provided for @theyLearn.
  ///
  /// In en, this message translates to:
  /// **'They will learn'**
  String get theyLearn;

  /// No description provided for @theyLearnPartly.
  ///
  /// In en, this message translates to:
  /// **'They may partly learn'**
  String get theyLearnPartly;

  /// No description provided for @staysPrivate.
  ///
  /// In en, this message translates to:
  /// **'Stays private'**
  String get staysPrivate;

  /// No description provided for @allowOnce.
  ///
  /// In en, this message translates to:
  /// **'Allow for this message'**
  String get allowOnce;

  /// No description provided for @allowContact.
  ///
  /// In en, this message translates to:
  /// **'Allow for this contact…'**
  String get allowContact;

  /// No description provided for @dontSend.
  ///
  /// In en, this message translates to:
  /// **'Don\'t send'**
  String get dontSend;

  /// No description provided for @confirmAlwaysTitle.
  ///
  /// In en, this message translates to:
  /// **'Allow without asking?'**
  String get confirmAlwaysTitle;

  /// No description provided for @confirmAlwaysBody.
  ///
  /// In en, this message translates to:
  /// **'Future messages to this contact may use this channel without this dialog, for the time you choose. Every such send still appears in the Privacy log, and you can revoke it in the contact\'s settings.'**
  String get confirmAlwaysBody;

  /// No description provided for @allow.
  ///
  /// In en, this message translates to:
  /// **'Allow'**
  String get allow;

  /// No description provided for @viaChannel.
  ///
  /// In en, this message translates to:
  /// **'via {channel}'**
  String viaChannel(String channel);

  /// No description provided for @channelQr.
  ///
  /// In en, this message translates to:
  /// **'QR'**
  String get channelQr;

  /// No description provided for @channelShare.
  ///
  /// In en, this message translates to:
  /// **'other app'**
  String get channelShare;

  /// No description provided for @contactSettings.
  ///
  /// In en, this message translates to:
  /// **'Contact'**
  String get contactSettings;

  /// No description provided for @privacyFloor.
  ///
  /// In en, this message translates to:
  /// **'Privacy floor'**
  String get privacyFloor;

  /// No description provided for @privacyFloorHelp.
  ///
  /// In en, this message translates to:
  /// **'Channels above this grade are used without asking. Lower grades always ask first and are recorded in the Privacy log.'**
  String get privacyFloorHelp;

  /// No description provided for @grade0.
  ///
  /// In en, this message translates to:
  /// **'P0 · in person (QR, radio)'**
  String get grade0;

  /// No description provided for @grade1.
  ///
  /// In en, this message translates to:
  /// **'P1 · direct network'**
  String get grade1;

  /// No description provided for @grade2.
  ///
  /// In en, this message translates to:
  /// **'P2 · own relay or hub'**
  String get grade2;

  /// No description provided for @grade3.
  ///
  /// In en, this message translates to:
  /// **'P3 · MEM couriers'**
  String get grade3;

  /// No description provided for @grade4.
  ///
  /// In en, this message translates to:
  /// **'P4 · other apps (encrypted)'**
  String get grade4;

  /// No description provided for @grade5.
  ///
  /// In en, this message translates to:
  /// **'P5 · plain text'**
  String get grade5;

  /// No description provided for @standingRules.
  ///
  /// In en, this message translates to:
  /// **'Standing permissions'**
  String get standingRules;

  /// No description provided for @noRules.
  ///
  /// In en, this message translates to:
  /// **'None'**
  String get noRules;

  /// No description provided for @revoke.
  ///
  /// In en, this message translates to:
  /// **'Revoke'**
  String get revoke;

  /// No description provided for @allContacts.
  ///
  /// In en, this message translates to:
  /// **'all contacts'**
  String get allContacts;

  /// No description provided for @logEmpty.
  ///
  /// In en, this message translates to:
  /// **'No message has gone below a privacy floor.'**
  String get logEmpty;

  /// No description provided for @basisOnce.
  ///
  /// In en, this message translates to:
  /// **'allowed once'**
  String get basisOnce;

  /// No description provided for @basisNewRule.
  ///
  /// In en, this message translates to:
  /// **'allowed, permission created'**
  String get basisNewRule;

  /// No description provided for @basisRule.
  ///
  /// In en, this message translates to:
  /// **'allowed by permission'**
  String get basisRule;

  /// No description provided for @logEntry.
  ///
  /// In en, this message translates to:
  /// **'{grade} below floor {floor} · {basis}'**
  String logEntry(String grade, String floor, String basis);

  /// No description provided for @yourName.
  ///
  /// In en, this message translates to:
  /// **'Your name (optional, shown on your code)'**
  String get yourName;

  /// No description provided for @save.
  ///
  /// In en, this message translates to:
  /// **'Save'**
  String get save;

  /// No description provided for @saved.
  ///
  /// In en, this message translates to:
  /// **'Saved'**
  String get saved;

  /// No description provided for @yourFingerprint.
  ///
  /// In en, this message translates to:
  /// **'Your fingerprint'**
  String get yourFingerprint;

  /// No description provided for @about.
  ///
  /// In en, this message translates to:
  /// **'About this build'**
  String get about;

  /// No description provided for @aboutBody.
  ///
  /// In en, this message translates to:
  /// **'MEM v0.2 — experimental. Messages are end-to-end encrypted (HPKE, Ed25519). They travel by animated QR code in person, directly over the same Wi-Fi network, or as encrypted text through another app. Courier and hub delivery come later. Do not rely on it for high-risk communication until an external security review.'**
  String get aboutBody;

  /// No description provided for @error.
  ///
  /// In en, this message translates to:
  /// **'Something went wrong: {details}'**
  String error(String details);

  /// No description provided for @reason_malformed.
  ///
  /// In en, this message translates to:
  /// **'This is not valid MEM data.'**
  String get reason_malformed;

  /// No description provided for @reason_unsupported.
  ///
  /// In en, this message translates to:
  /// **'Made by a newer MEM version.'**
  String get reason_unsupported;

  /// No description provided for @reason_not_for_us.
  ///
  /// In en, this message translates to:
  /// **'Not addressed to you, or from someone you haven\'t added.'**
  String get reason_not_for_us;

  /// No description provided for @reason_undecryptable.
  ///
  /// In en, this message translates to:
  /// **'Could not be decrypted. Ask the sender to pair again.'**
  String get reason_undecryptable;

  /// No description provided for @reason_bad_signature.
  ///
  /// In en, this message translates to:
  /// **'The sender could not be verified.'**
  String get reason_bad_signature;

  /// No description provided for @reason_duplicate.
  ///
  /// In en, this message translates to:
  /// **'You already have this message.'**
  String get reason_duplicate;

  /// No description provided for @reason_purged.
  ///
  /// In en, this message translates to:
  /// **'You already received this message (or the sender withdrew it).'**
  String get reason_purged;

  /// No description provided for @reason_expired.
  ///
  /// In en, this message translates to:
  /// **'This message has expired.'**
  String get reason_expired;

  /// No description provided for @reason_own_card.
  ///
  /// In en, this message translates to:
  /// **'That\'s your own code.'**
  String get reason_own_card;

  /// No description provided for @reason_unknown_contact.
  ///
  /// In en, this message translates to:
  /// **'Unknown contact.'**
  String get reason_unknown_contact;

  /// No description provided for @reason_unknown_message.
  ///
  /// In en, this message translates to:
  /// **'This message can no longer be sent.'**
  String get reason_unknown_message;

  /// No description provided for @reason_too_large_for_channel.
  ///
  /// In en, this message translates to:
  /// **'Too long for this channel.'**
  String get reason_too_large_for_channel;

  /// No description provided for @reason_consent_mismatch.
  ///
  /// In en, this message translates to:
  /// **'Permission did not match. Please try again.'**
  String get reason_consent_mismatch;

  /// No description provided for @reason_invalid_text.
  ///
  /// In en, this message translates to:
  /// **'The message is empty or too long.'**
  String get reason_invalid_text;

  /// No description provided for @reason_file_too_large.
  ///
  /// In en, this message translates to:
  /// **'This file is too large. MEM sends files up to 100 MB, at most 16 at a time.'**
  String get reason_file_too_large;

  /// No description provided for @reason_invalid_attachment.
  ///
  /// In en, this message translates to:
  /// **'This file cannot be attached: its name or type is too long.'**
  String get reason_invalid_attachment;

  /// No description provided for @attachmentSending.
  ///
  /// In en, this message translates to:
  /// **'Sending {sent} of {total}'**
  String attachmentSending(int sent, int total);

  /// No description provided for @attachmentArriving.
  ///
  /// In en, this message translates to:
  /// **'Arriving {have} of {total}'**
  String attachmentArriving(int have, int total);

  /// No description provided for @attachmentMissing.
  ///
  /// In en, this message translates to:
  /// **'This file is not on this device.'**
  String get attachmentMissing;

  /// No description provided for @attachmentOpenElsewhere.
  ///
  /// In en, this message translates to:
  /// **'Open in another app'**
  String get attachmentOpenElsewhere;

  /// No description provided for @attachmentOpenElsewhereBody.
  ///
  /// In en, this message translates to:
  /// **'The app you choose gets this file, unencrypted, and may keep or upload it.'**
  String get attachmentOpenElsewhereBody;

  /// No description provided for @attachmentOpenElsewhereAction.
  ///
  /// In en, this message translates to:
  /// **'Choose app'**
  String get attachmentOpenElsewhereAction;

  /// No description provided for @playerPlay.
  ///
  /// In en, this message translates to:
  /// **'Play'**
  String get playerPlay;

  /// No description provided for @playerPause.
  ///
  /// In en, this message translates to:
  /// **'Pause'**
  String get playerPause;

  /// No description provided for @playerCannotPlay.
  ///
  /// In en, this message translates to:
  /// **'This file cannot be played here. Open it in another app with the share button.'**
  String get playerCannotPlay;

  /// No description provided for @mediaPlayer.
  ///
  /// In en, this message translates to:
  /// **'Built-in media player'**
  String get mediaPlayer;

  /// No description provided for @mediaPlayerHelp.
  ///
  /// In en, this message translates to:
  /// **'Video and audio files play inside MEM: they are decrypted into a private temporary file that is deleted when the player closes. Off, they open in another app, which then gets the file.'**
  String get mediaPlayerHelp;

  /// No description provided for @voiceCalls.
  ///
  /// In en, this message translates to:
  /// **'Voice calls (test)'**
  String get voiceCalls;

  /// No description provided for @voiceCallsHelp.
  ///
  /// In en, this message translates to:
  /// **'Call contacts over Wi-Fi Direct or the local network. Call setup travels end to end through MEM; the voice is encrypted. The contact learns your address on that network. Being tested: off by default.'**
  String get voiceCallsHelp;

  /// No description provided for @callStart.
  ///
  /// In en, this message translates to:
  /// **'Call'**
  String get callStart;

  /// No description provided for @callCalling.
  ///
  /// In en, this message translates to:
  /// **'Calling…'**
  String get callCalling;

  /// No description provided for @callIncoming.
  ///
  /// In en, this message translates to:
  /// **'Incoming call'**
  String get callIncoming;

  /// No description provided for @callConnecting.
  ///
  /// In en, this message translates to:
  /// **'Connecting…'**
  String get callConnecting;

  /// No description provided for @callAccept.
  ///
  /// In en, this message translates to:
  /// **'Accept'**
  String get callAccept;

  /// No description provided for @callDecline.
  ///
  /// In en, this message translates to:
  /// **'Decline'**
  String get callDecline;

  /// No description provided for @callMute.
  ///
  /// In en, this message translates to:
  /// **'Mute'**
  String get callMute;

  /// No description provided for @callUnmute.
  ///
  /// In en, this message translates to:
  /// **'Unmute'**
  String get callUnmute;

  /// No description provided for @callSpeaker.
  ///
  /// In en, this message translates to:
  /// **'Speaker'**
  String get callSpeaker;

  /// No description provided for @callHangUp.
  ///
  /// In en, this message translates to:
  /// **'Hang up'**
  String get callHangUp;

  /// No description provided for @callEnded.
  ///
  /// In en, this message translates to:
  /// **'Call ended'**
  String get callEnded;

  /// No description provided for @callDeclined.
  ///
  /// In en, this message translates to:
  /// **'Declined'**
  String get callDeclined;

  /// No description provided for @callBusy.
  ///
  /// In en, this message translates to:
  /// **'Busy in another call'**
  String get callBusy;

  /// No description provided for @callFailed.
  ///
  /// In en, this message translates to:
  /// **'The call could not connect'**
  String get callFailed;

  /// No description provided for @callUnanswered.
  ///
  /// In en, this message translates to:
  /// **'No answer'**
  String get callUnanswered;

  /// No description provided for @callNoMicrophone.
  ///
  /// In en, this message translates to:
  /// **'Calls need the microphone. Allow it in the phone\'s settings.'**
  String get callNoMicrophone;

  /// No description provided for @toneIncoming.
  ///
  /// In en, this message translates to:
  /// **'Incoming call tone'**
  String get toneIncoming;

  /// No description provided for @toneOutgoing.
  ///
  /// In en, this message translates to:
  /// **'Ringback while calling'**
  String get toneOutgoing;

  /// No description provided for @tonePreview.
  ///
  /// In en, this message translates to:
  /// **'Listen'**
  String get tonePreview;

  /// No description provided for @toneOff.
  ///
  /// In en, this message translates to:
  /// **'Off'**
  String get toneOff;

  /// No description provided for @toneDefault.
  ///
  /// In en, this message translates to:
  /// **'Default'**
  String get toneDefault;

  /// No description provided for @contactRingtone.
  ///
  /// In en, this message translates to:
  /// **'Ringtone'**
  String get contactRingtone;

  /// No description provided for @toneChime.
  ///
  /// In en, this message translates to:
  /// **'Chime'**
  String get toneChime;

  /// No description provided for @toneClassic.
  ///
  /// In en, this message translates to:
  /// **'Classic'**
  String get toneClassic;

  /// No description provided for @tonePulse.
  ///
  /// In en, this message translates to:
  /// **'Pulse'**
  String get tonePulse;

  /// No description provided for @toneSoft.
  ///
  /// In en, this message translates to:
  /// **'Soft'**
  String get toneSoft;

  /// No description provided for @toneRingback.
  ///
  /// In en, this message translates to:
  /// **'Standard'**
  String get toneRingback;

  /// No description provided for @toneBeep.
  ///
  /// In en, this message translates to:
  /// **'Short beep'**
  String get toneBeep;

  /// No description provided for @callHistory.
  ///
  /// In en, this message translates to:
  /// **'Keep call history in chats'**
  String get callHistory;

  /// No description provided for @callHistoryHelp.
  ///
  /// In en, this message translates to:
  /// **'Each call appears in the chat with its length and path. Stored only on this device.'**
  String get callHistoryHelp;

  /// No description provided for @callLineOutgoing.
  ///
  /// In en, this message translates to:
  /// **'Outgoing call'**
  String get callLineOutgoing;

  /// No description provided for @callLineIncoming.
  ///
  /// In en, this message translates to:
  /// **'Incoming call'**
  String get callLineIncoming;

  /// No description provided for @callLineMissed.
  ///
  /// In en, this message translates to:
  /// **'Missed call'**
  String get callLineMissed;

  /// No description provided for @reason_invalid_name.
  ///
  /// In en, this message translates to:
  /// **'The name is too long.'**
  String get reason_invalid_name;

  /// No description provided for @channelLan.
  ///
  /// In en, this message translates to:
  /// **'Wi-Fi'**
  String get channelLan;

  /// No description provided for @nearby.
  ///
  /// In en, this message translates to:
  /// **'nearby'**
  String get nearby;

  /// No description provided for @nearbyDevices.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =0{No MEM devices nearby} =1{1 MEM device nearby} other{{count} MEM devices nearby}}'**
  String nearbyDevices(int count);

  /// No description provided for @lanOffShort.
  ///
  /// In en, this message translates to:
  /// **'Wi-Fi delivery off'**
  String get lanOffShort;

  /// No description provided for @notNearby.
  ///
  /// In en, this message translates to:
  /// **'Not nearby now'**
  String get notNearby;

  /// No description provided for @sendingNearby.
  ///
  /// In en, this message translates to:
  /// **'Sending to a device nearby…'**
  String get sendingNearby;

  /// No description provided for @waitForWifi.
  ///
  /// In en, this message translates to:
  /// **'Wait until nearby'**
  String get waitForWifi;

  /// No description provided for @waitForWifiHint.
  ///
  /// In en, this message translates to:
  /// **'Sent automatically when you are on the same Wi-Fi, or next to each other with Bluetooth on'**
  String get waitForWifiHint;

  /// No description provided for @channels.
  ///
  /// In en, this message translates to:
  /// **'Channels'**
  String get channels;

  /// No description provided for @channelsIntro.
  ///
  /// In en, this message translates to:
  /// **'What others can learn when MEM uses each channel. Message contents are always end-to-end encrypted, except a P5 fallback you confirm per message.'**
  String get channelsIntro;

  /// No description provided for @lanToggle.
  ///
  /// In en, this message translates to:
  /// **'Deliver over the local network'**
  String get lanToggle;

  /// No description provided for @lanToggleHelp.
  ///
  /// In en, this message translates to:
  /// **'While on, other devices on the same Wi-Fi can see that this device runs MEM and its network addresses. Turn off on networks you don\'t trust.'**
  String get lanToggleHelp;

  /// No description provided for @scanProgress.
  ///
  /// In en, this message translates to:
  /// **'Receiving… {received} of {needed}'**
  String scanProgress(int received, int needed);

  /// No description provided for @animatedQrHint.
  ///
  /// In en, this message translates to:
  /// **'Keep this on screen until your contact\'s phone has it. Missed frames don\'t matter: any {needed} are enough.'**
  String animatedQrHint(int needed);

  /// No description provided for @reason_unreachable.
  ///
  /// In en, this message translates to:
  /// **'Your contact is not nearby right now.'**
  String get reason_unreachable;

  /// No description provided for @reason_channel_disabled.
  ///
  /// In en, this message translates to:
  /// **'This channel is turned off in Channels.'**
  String get reason_channel_disabled;

  /// No description provided for @reason_quota.
  ///
  /// In en, this message translates to:
  /// **'Too much data from a nearby device; disconnected.'**
  String get reason_quota;

  /// No description provided for @reason_unknown.
  ///
  /// In en, this message translates to:
  /// **'Something unexpected happened.'**
  String get reason_unknown;

  /// No description provided for @appearance.
  ///
  /// In en, this message translates to:
  /// **'Appearance'**
  String get appearance;

  /// No description provided for @themeSystem.
  ///
  /// In en, this message translates to:
  /// **'System'**
  String get themeSystem;

  /// No description provided for @themeWhite.
  ///
  /// In en, this message translates to:
  /// **'White'**
  String get themeWhite;

  /// No description provided for @themeGray.
  ///
  /// In en, this message translates to:
  /// **'Gray'**
  String get themeGray;

  /// No description provided for @themeBlack.
  ///
  /// In en, this message translates to:
  /// **'Black'**
  String get themeBlack;

  /// No description provided for @notConfirmed.
  ///
  /// In en, this message translates to:
  /// **'Not confirmed yet — tap to send again'**
  String get notConfirmed;

  /// No description provided for @awaitingConfirmation.
  ///
  /// In en, this message translates to:
  /// **'Waiting for confirmation'**
  String get awaitingConfirmation;

  /// No description provided for @retryTitle.
  ///
  /// In en, this message translates to:
  /// **'Retry unconfirmed messages'**
  String get retryTitle;

  /// No description provided for @retryDays.
  ///
  /// In en, this message translates to:
  /// **'{days, plural, =1{1 day} other{{days} days}}'**
  String retryDays(int days);

  /// No description provided for @retryHelp.
  ///
  /// In en, this message translates to:
  /// **'Messages your contact has not confirmed are sent again whenever you are connected, for up to this long. Applies to new messages.'**
  String get retryHelp;

  /// No description provided for @backupTitle.
  ///
  /// In en, this message translates to:
  /// **'Backup and recovery'**
  String get backupTitle;

  /// No description provided for @backupIntro.
  ///
  /// In en, this message translates to:
  /// **'A backup holds your identity, contacts, keys and, if you choose, your messages. It is encrypted: only your recovery phrase opens it. MEM keeps no copy anywhere.'**
  String get backupIntro;

  /// No description provided for @backupOffBody.
  ///
  /// In en, this message translates to:
  /// **'Turn on backups to get a recovery phrase. Write it on paper and keep it apart from your backup files: anyone who has both can read your messages and write as you.'**
  String get backupOffBody;

  /// No description provided for @phraseFormTitle.
  ///
  /// In en, this message translates to:
  /// **'Recovery phrase'**
  String get phraseFormTitle;

  /// No description provided for @formEnglish.
  ///
  /// In en, this message translates to:
  /// **'English words'**
  String get formEnglish;

  /// No description provided for @formSyllables.
  ///
  /// In en, this message translates to:
  /// **'Latin syllables'**
  String get formSyllables;

  /// No description provided for @turnOnBackups.
  ///
  /// In en, this message translates to:
  /// **'Turn on backups'**
  String get turnOnBackups;

  /// No description provided for @phraseTitle.
  ///
  /// In en, this message translates to:
  /// **'Your recovery phrase'**
  String get phraseTitle;

  /// No description provided for @phraseHint.
  ///
  /// In en, this message translates to:
  /// **'Write these down in order, on paper. Screenshots are blocked on this screen.'**
  String get phraseHint;

  /// No description provided for @phraseWritten.
  ///
  /// In en, this message translates to:
  /// **'I have written it down'**
  String get phraseWritten;

  /// No description provided for @confirmTitle.
  ///
  /// In en, this message translates to:
  /// **'Check your phrase'**
  String get confirmTitle;

  /// No description provided for @confirmHint.
  ///
  /// In en, this message translates to:
  /// **'Type these words from your note.'**
  String get confirmHint;

  /// No description provided for @confirmWrong.
  ///
  /// In en, this message translates to:
  /// **'That does not match. Look at your note again.'**
  String get confirmWrong;

  /// No description provided for @confirmDone.
  ///
  /// In en, this message translates to:
  /// **'Backups are on.'**
  String get confirmDone;

  /// No description provided for @wordN.
  ///
  /// In en, this message translates to:
  /// **'Word {n}'**
  String wordN(int n);

  /// No description provided for @includeHistory.
  ///
  /// In en, this message translates to:
  /// **'Include messages'**
  String get includeHistory;

  /// No description provided for @saveBackupFile.
  ///
  /// In en, this message translates to:
  /// **'Save backup file'**
  String get saveBackupFile;

  /// No description provided for @backupSaved.
  ///
  /// In en, this message translates to:
  /// **'Backup saved'**
  String get backupSaved;

  /// No description provided for @showBackupQr.
  ///
  /// In en, this message translates to:
  /// **'Show as QR code'**
  String get showBackupQr;

  /// No description provided for @backupQrHint.
  ///
  /// In en, this message translates to:
  /// **'On the new phone, open MEM → Profile → Backup and recovery → Restore from backup, and point it at this screen.'**
  String get backupQrHint;

  /// No description provided for @backupTooLargeForQr.
  ///
  /// In en, this message translates to:
  /// **'Too large for a QR code with messages: showing it without messages.'**
  String get backupTooLargeForQr;

  /// No description provided for @shareBackup.
  ///
  /// In en, this message translates to:
  /// **'Send through another app…'**
  String get shareBackup;

  /// No description provided for @shareBackupWarning.
  ///
  /// In en, this message translates to:
  /// **'The app you choose will see that you have a MEM backup, roughly how large it is, and when you sent it. It cannot read it.'**
  String get shareBackupWarning;

  /// No description provided for @showPhrase.
  ///
  /// In en, this message translates to:
  /// **'Show recovery phrase'**
  String get showPhrase;

  /// No description provided for @newPhrase.
  ///
  /// In en, this message translates to:
  /// **'New recovery phrase'**
  String get newPhrase;

  /// No description provided for @newPhraseWarning.
  ///
  /// In en, this message translates to:
  /// **'Backups you make from now on need the new phrase. Older backup files still open only with the old phrase.'**
  String get newPhraseWarning;

  /// No description provided for @scheduleTitle.
  ///
  /// In en, this message translates to:
  /// **'Automatic backups'**
  String get scheduleTitle;

  /// No description provided for @scheduleOff.
  ///
  /// In en, this message translates to:
  /// **'Off'**
  String get scheduleOff;

  /// No description provided for @scheduleEveryDays.
  ///
  /// In en, this message translates to:
  /// **'{days, plural, =1{Every day} other{Every {days} days}}'**
  String scheduleEveryDays(int days);

  /// No description provided for @scheduleAfterContact.
  ///
  /// In en, this message translates to:
  /// **'After each new contact'**
  String get scheduleAfterContact;

  /// No description provided for @backupFolder.
  ///
  /// In en, this message translates to:
  /// **'Folder'**
  String get backupFolder;

  /// No description provided for @chooseFolder.
  ///
  /// In en, this message translates to:
  /// **'Choose'**
  String get chooseFolder;

  /// No description provided for @noFolder.
  ///
  /// In en, this message translates to:
  /// **'No folder chosen'**
  String get noFolder;

  /// No description provided for @cloudFolderNote.
  ///
  /// In en, this message translates to:
  /// **'If this folder syncs to a cloud service, that service sees each backup\'s size and time. It cannot read them.'**
  String get cloudFolderNote;

  /// No description provided for @lastBackup.
  ///
  /// In en, this message translates to:
  /// **'Last backup: {when}'**
  String lastBackup(String when);

  /// No description provided for @backupFailed.
  ///
  /// In en, this message translates to:
  /// **'The last automatic backup failed.'**
  String get backupFailed;

  /// No description provided for @restoreTitle.
  ///
  /// In en, this message translates to:
  /// **'Restore from backup'**
  String get restoreTitle;

  /// No description provided for @restoreHint.
  ///
  /// In en, this message translates to:
  /// **'Scan the QR code from your old phone, open a backup file, or paste backup text. Then type your recovery phrase.'**
  String get restoreHint;

  /// No description provided for @openBackupFile.
  ///
  /// In en, this message translates to:
  /// **'Open backup file'**
  String get openBackupFile;

  /// No description provided for @pasteBackup.
  ///
  /// In en, this message translates to:
  /// **'Paste backup text'**
  String get pasteBackup;

  /// No description provided for @backupReceived.
  ///
  /// In en, this message translates to:
  /// **'Backup received. Type your recovery phrase.'**
  String get backupReceived;

  /// No description provided for @phraseInput.
  ///
  /// In en, this message translates to:
  /// **'Recovery phrase'**
  String get phraseInput;

  /// No description provided for @restoreAction.
  ///
  /// In en, this message translates to:
  /// **'Restore'**
  String get restoreAction;

  /// No description provided for @restoreReplaceTitle.
  ///
  /// In en, this message translates to:
  /// **'Replace this identity?'**
  String get restoreReplaceTitle;

  /// No description provided for @restoreReplaceBody.
  ///
  /// In en, this message translates to:
  /// **'This phone already has a MEM identity with contacts. Restoring replaces it. The current state is kept, encrypted, in the app\'s storage; it is not deleted.'**
  String get restoreReplaceBody;

  /// No description provided for @restoreDone.
  ///
  /// In en, this message translates to:
  /// **'Restored. If you still have your old phone, reset MEM there: until then, both can read new messages.'**
  String get restoreDone;

  /// No description provided for @restoreCorrected.
  ///
  /// In en, this message translates to:
  /// **'One letter in your phrase was corrected: {phrase}'**
  String restoreCorrected(String phrase);

  /// No description provided for @restoreWrongPhrase.
  ///
  /// In en, this message translates to:
  /// **'This phrase does not open this backup.'**
  String get restoreWrongPhrase;

  /// No description provided for @restoreNotBackup.
  ///
  /// In en, this message translates to:
  /// **'This is not a MEM backup.'**
  String get restoreNotBackup;

  /// No description provided for @working.
  ///
  /// In en, this message translates to:
  /// **'Working…'**
  String get working;

  /// No description provided for @qrPaceEasy.
  ///
  /// In en, this message translates to:
  /// **'Easy'**
  String get qrPaceEasy;

  /// No description provided for @qrPaceNormal.
  ///
  /// In en, this message translates to:
  /// **'Normal'**
  String get qrPaceNormal;

  /// No description provided for @qrPaceFast.
  ///
  /// In en, this message translates to:
  /// **'Fast'**
  String get qrPaceFast;

  /// No description provided for @qrPaceHint.
  ///
  /// In en, this message translates to:
  /// **'If the other phone can\'t read the code, choose Easy.'**
  String get qrPaceHint;

  /// No description provided for @settings.
  ///
  /// In en, this message translates to:
  /// **'Settings'**
  String get settings;

  /// No description provided for @language.
  ///
  /// In en, this message translates to:
  /// **'Language'**
  String get language;

  /// No description provided for @followSystem.
  ///
  /// In en, this message translates to:
  /// **'System'**
  String get followSystem;

  /// No description provided for @textSize.
  ///
  /// In en, this message translates to:
  /// **'Use the phone\'s text size'**
  String get textSize;

  /// No description provided for @percent.
  ///
  /// In en, this message translates to:
  /// **'{value} %'**
  String percent(int value);

  /// No description provided for @sectionMessages.
  ///
  /// In en, this message translates to:
  /// **'Messages'**
  String get sectionMessages;

  /// No description provided for @showMessageTimes.
  ///
  /// In en, this message translates to:
  /// **'Show message times'**
  String get showMessageTimes;

  /// No description provided for @badgeDetail.
  ///
  /// In en, this message translates to:
  /// **'Channel badges'**
  String get badgeDetail;

  /// No description provided for @badgeCompact.
  ///
  /// In en, this message translates to:
  /// **'Compact'**
  String get badgeCompact;

  /// No description provided for @badgeDetailed.
  ///
  /// In en, this message translates to:
  /// **'Detailed'**
  String get badgeDetailed;

  /// No description provided for @badgeDetailHelp.
  ///
  /// In en, this message translates to:
  /// **'Detailed full badges also show when each channel carried the message, and whether it was delivered.'**
  String get badgeDetailHelp;

  /// No description provided for @messageBadges.
  ///
  /// In en, this message translates to:
  /// **'Message badges'**
  String get messageBadges;

  /// No description provided for @messageBadgesHelp.
  ///
  /// In en, this message translates to:
  /// **'Channel badges sit at the bottom left of each message, on the line of its time. They can shrink to a symbol after a while; tap the badge area to see them in full. Other apps (P4), plain text (P5) and channels below the contact\'s privacy floor always keep their symbol.'**
  String get messageBadgesHelp;

  /// No description provided for @badgeModeMinimal.
  ///
  /// In en, this message translates to:
  /// **'Symbols only for P4, P5 and below the floor'**
  String get badgeModeMinimal;

  /// No description provided for @badgeModeSymbols.
  ///
  /// In en, this message translates to:
  /// **'Symbols only'**
  String get badgeModeSymbols;

  /// No description provided for @badgeModeCollapse.
  ///
  /// In en, this message translates to:
  /// **'Full, then symbols after {delay}'**
  String badgeModeCollapse(String delay);

  /// No description provided for @badgeModeFull.
  ///
  /// In en, this message translates to:
  /// **'Always full'**
  String get badgeModeFull;

  /// No description provided for @durationSeconds.
  ///
  /// In en, this message translates to:
  /// **'{n} s'**
  String durationSeconds(int n);

  /// No description provided for @durationMinutes.
  ///
  /// In en, this message translates to:
  /// **'{n} min'**
  String durationMinutes(int n);

  /// No description provided for @durationHours.
  ///
  /// In en, this message translates to:
  /// **'{n} h'**
  String durationHours(int n);

  /// No description provided for @badgeSymbols.
  ///
  /// In en, this message translates to:
  /// **'Badge symbols'**
  String get badgeSymbols;

  /// No description provided for @badgeSymbolsByGrade.
  ///
  /// In en, this message translates to:
  /// **'By grade'**
  String get badgeSymbolsByGrade;

  /// No description provided for @badgeSymbolsOneShape.
  ///
  /// In en, this message translates to:
  /// **'One shape'**
  String get badgeSymbolsOneShape;

  /// No description provided for @badgeSymbolsHelp.
  ///
  /// In en, this message translates to:
  /// **'By grade gives each privacy grade its own shape. One shape uses the shape you pick for P0 to P3; P4 and P5 always keep their own.'**
  String get badgeSymbolsHelp;

  /// No description provided for @sectionQr.
  ///
  /// In en, this message translates to:
  /// **'QR codes'**
  String get sectionQr;

  /// No description provided for @qrPaceTitle.
  ///
  /// In en, this message translates to:
  /// **'Animated QR speed'**
  String get qrPaceTitle;

  /// No description provided for @sectionPrivacy.
  ///
  /// In en, this message translates to:
  /// **'Privacy'**
  String get sectionPrivacy;

  /// No description provided for @defaultFloor.
  ///
  /// In en, this message translates to:
  /// **'Privacy floor for new contacts'**
  String get defaultFloor;

  /// No description provided for @defaultFloorHelp.
  ///
  /// In en, this message translates to:
  /// **'Channels below the floor need your permission before a message goes out. Contacts you already have keep their own floor; change it on the contact\'s page.'**
  String get defaultFloorHelp;

  /// No description provided for @blockScreenshots.
  ///
  /// In en, this message translates to:
  /// **'Block screenshots'**
  String get blockScreenshots;

  /// No description provided for @blockScreenshotsHelp.
  ///
  /// In en, this message translates to:
  /// **'No screenshots or screen recording, and MEM\'s content is hidden in the recent apps list. The recovery phrase is always protected.'**
  String get blockScreenshotsHelp;

  /// No description provided for @showNetworkStatus.
  ///
  /// In en, this message translates to:
  /// **'Show network status on the main screen'**
  String get showNetworkStatus;

  /// No description provided for @statusDelivered.
  ///
  /// In en, this message translates to:
  /// **'Delivered'**
  String get statusDelivered;

  /// No description provided for @statusReceived.
  ///
  /// In en, this message translates to:
  /// **'Received'**
  String get statusReceived;

  /// No description provided for @reason_invalid_floor.
  ///
  /// In en, this message translates to:
  /// **'P5 cannot be a privacy floor: plaintext always needs your permission for each message.'**
  String get reason_invalid_floor;

  /// No description provided for @phraseNotConfirmed.
  ///
  /// In en, this message translates to:
  /// **'The new phrase was not confirmed, so nothing changed.'**
  String get phraseNotConfirmed;

  /// No description provided for @scanFocusHint.
  ///
  /// In en, this message translates to:
  /// **'Blurry? Tap the picture to focus.'**
  String get scanFocusHint;

  /// No description provided for @helpTooltip.
  ///
  /// In en, this message translates to:
  /// **'How it works'**
  String get helpTooltip;

  /// No description provided for @scanBrightness.
  ///
  /// In en, this message translates to:
  /// **'Brightness of the camera picture'**
  String get scanBrightness;

  /// No description provided for @codeBrightness.
  ///
  /// In en, this message translates to:
  /// **'Screen brightness for the code'**
  String get codeBrightness;

  /// No description provided for @stayReachable.
  ///
  /// In en, this message translates to:
  /// **'Stay reachable in the background'**
  String get stayReachable;

  /// No description provided for @stayReachableHelp.
  ///
  /// In en, this message translates to:
  /// **'Keeps Wi-Fi and Bluetooth delivery working with the screen off. Shows a permanent notification, so anyone who sees your phone can tell MEM is running, and uses more battery.'**
  String get stayReachableHelp;

  /// No description provided for @reachableNotification.
  ///
  /// In en, this message translates to:
  /// **'Reachable over Wi-Fi'**
  String get reachableNotification;

  /// No description provided for @batteryRestricted.
  ///
  /// In en, this message translates to:
  /// **'Android may still pause MEM to save battery. Tap to let it run unrestricted.'**
  String get batteryRestricted;

  /// No description provided for @batteryUnrestricted.
  ///
  /// In en, this message translates to:
  /// **'MEM may run unrestricted.'**
  String get batteryUnrestricted;

  /// No description provided for @sectionCourier.
  ///
  /// In en, this message translates to:
  /// **'Courier'**
  String get sectionCourier;

  /// No description provided for @courierMode.
  ///
  /// In en, this message translates to:
  /// **'Carry messages for other MEM users'**
  String get courierMode;

  /// No description provided for @courierModeHelp.
  ///
  /// In en, this message translates to:
  /// **'Your phone keeps sealed messages for people nearby and passes them on when it meets their phones. It cannot read them: it sees only scrambled addresses, sizes and times. Uses storage and some battery.'**
  String get courierModeHelp;

  /// No description provided for @courierLimit.
  ///
  /// In en, this message translates to:
  /// **'Up to {mb} MB'**
  String courierLimit(int mb);

  /// No description provided for @courierUsage.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{Carrying 1 message} other{Carrying {count} messages}} · {used} of {limit} MB'**
  String courierUsage(int count, String used, String limit);

  /// No description provided for @channelCourier.
  ///
  /// In en, this message translates to:
  /// **'couriers'**
  String get channelCourier;

  /// No description provided for @courierQueued.
  ///
  /// In en, this message translates to:
  /// **'It will travel with the next MEM device you meet.'**
  String get courierQueued;

  /// No description provided for @courierHint.
  ///
  /// In en, this message translates to:
  /// **'Goes with nearby MEM devices that cannot read it.'**
  String get courierHint;

  /// No description provided for @reason_waiting_for_key.
  ///
  /// In en, this message translates to:
  /// **'Waiting for your contact\'s new encryption key. Any message from them brings it, and this message then goes out by itself.'**
  String get reason_waiting_for_key;

  /// No description provided for @requestTab.
  ///
  /// In en, this message translates to:
  /// **'Request'**
  String get requestTab;

  /// No description provided for @requestHint.
  ///
  /// In en, this message translates to:
  /// **'For someone you cannot meet: paste their MEM code. They get a contact request with your code and your note, and decide whether to accept. It travels through nearby MEM devices, so it can take a while.'**
  String get requestHint;

  /// No description provided for @requestCodeLabel.
  ///
  /// In en, this message translates to:
  /// **'Their code (memcard1:…)'**
  String get requestCodeLabel;

  /// No description provided for @requestNoteLabel.
  ///
  /// In en, this message translates to:
  /// **'Note (optional)'**
  String get requestNoteLabel;

  /// No description provided for @requestNoteHelp.
  ///
  /// In en, this message translates to:
  /// **'Shown with your request: who you are and how they know you.'**
  String get requestNoteHelp;

  /// No description provided for @sendRequest.
  ///
  /// In en, this message translates to:
  /// **'Send request'**
  String get sendRequest;

  /// No description provided for @requestPreparing.
  ///
  /// In en, this message translates to:
  /// **'Preparing the request. This takes a few seconds.'**
  String get requestPreparing;

  /// No description provided for @requestPendingBanner.
  ///
  /// In en, this message translates to:
  /// **'Waiting for {name} to accept your contact request. You can write once they do.'**
  String requestPendingBanner(String name);

  /// No description provided for @contactRequestLabel.
  ///
  /// In en, this message translates to:
  /// **'Contact request'**
  String get contactRequestLabel;

  /// No description provided for @requestNoNote.
  ///
  /// In en, this message translates to:
  /// **'(no note)'**
  String get requestNoNote;

  /// No description provided for @requestsTitle.
  ///
  /// In en, this message translates to:
  /// **'Contact requests'**
  String get requestsTitle;

  /// No description provided for @requestsTooltip.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 contact request} other{{count} contact requests}}'**
  String requestsTooltip(int count);

  /// No description provided for @requestsEmpty.
  ///
  /// In en, this message translates to:
  /// **'No contact requests.'**
  String get requestsEmpty;

  /// No description provided for @requestsHelp.
  ///
  /// In en, this message translates to:
  /// **'These people have your MEM code and want to become contacts. Names are not verified: compare the safety symbols when you meet.'**
  String get requestsHelp;

  /// No description provided for @nameNotVerified.
  ///
  /// In en, this message translates to:
  /// **'{name} (name not verified)'**
  String nameNotVerified(String name);

  /// No description provided for @acceptRequest.
  ///
  /// In en, this message translates to:
  /// **'Accept'**
  String get acceptRequest;

  /// No description provided for @declineRequest.
  ///
  /// In en, this message translates to:
  /// **'Decline'**
  String get declineRequest;

  /// No description provided for @blockRequest.
  ///
  /// In en, this message translates to:
  /// **'Block'**
  String get blockRequest;

  /// No description provided for @blockConfirm.
  ///
  /// In en, this message translates to:
  /// **'Block this sender? Their contact requests will be ignored from now on.'**
  String get blockConfirm;

  /// No description provided for @requestAccepted.
  ///
  /// In en, this message translates to:
  /// **'Contact added'**
  String get requestAccepted;

  /// No description provided for @requestReceived.
  ///
  /// In en, this message translates to:
  /// **'Contact request received'**
  String get requestReceived;

  /// No description provided for @requestsNearby.
  ///
  /// In en, this message translates to:
  /// **'Accept contact requests from people you haven\'t met'**
  String get requestsNearby;

  /// No description provided for @requestsNearbyHelp.
  ///
  /// In en, this message translates to:
  /// **'Requests come through nearby MEM devices and, if you use one, the hub. Anyone who has your MEM code could notice when your phone is nearby, and the hub could link your internet address to your code. Requests you paste or scan arrive either way.'**
  String get requestsNearbyHelp;

  /// No description provided for @requestedSubtitle.
  ///
  /// In en, this message translates to:
  /// **'Request sent · waiting for an answer'**
  String get requestedSubtitle;

  /// No description provided for @reason_awaiting_approval.
  ///
  /// In en, this message translates to:
  /// **'They have not accepted your contact request yet.'**
  String get reason_awaiting_approval;

  /// No description provided for @reason_bad_proof_of_work.
  ///
  /// In en, this message translates to:
  /// **'This contact request is not valid.'**
  String get reason_bad_proof_of_work;

  /// No description provided for @reason_unknown_request.
  ///
  /// In en, this message translates to:
  /// **'This request was already answered.'**
  String get reason_unknown_request;

  /// No description provided for @reason_already_contact.
  ///
  /// In en, this message translates to:
  /// **'You are already contacts, or a request is on its way.'**
  String get reason_already_contact;

  /// No description provided for @reason_invalid_note.
  ///
  /// In en, this message translates to:
  /// **'The note is too long (at most 280 bytes).'**
  String get reason_invalid_note;

  /// No description provided for @reason_not_a_card.
  ///
  /// In en, this message translates to:
  /// **'That is not a MEM code.'**
  String get reason_not_a_card;

  /// No description provided for @channelBle.
  ///
  /// In en, this message translates to:
  /// **'Bluetooth'**
  String get channelBle;

  /// No description provided for @bleToggle.
  ///
  /// In en, this message translates to:
  /// **'Deliver over Bluetooth'**
  String get bleToggle;

  /// No description provided for @bleToggleHelp.
  ///
  /// In en, this message translates to:
  /// **'Phones next to each other exchange messages without any network. While on, any Bluetooth scanner nearby can tell that a MEM device is here, though not whose.'**
  String get bleToggleHelp;

  /// No description provided for @bleLocationExplain.
  ///
  /// In en, this message translates to:
  /// **'On this Android version, Android lets apps look for Bluetooth devices only with the location permission and with Location switched on. MEM never reads your location.'**
  String get bleLocationExplain;

  /// No description provided for @bleStatusRunning.
  ///
  /// In en, this message translates to:
  /// **'Looking for MEM devices nearby'**
  String get bleStatusRunning;

  /// No description provided for @bleStatusOff.
  ///
  /// In en, this message translates to:
  /// **'Bluetooth is switched off'**
  String get bleStatusOff;

  /// No description provided for @bleStatusPermission.
  ///
  /// In en, this message translates to:
  /// **'Bluetooth permission is missing'**
  String get bleStatusPermission;

  /// No description provided for @bleStatusLocation.
  ///
  /// In en, this message translates to:
  /// **'Location is off, so Android hides nearby devices'**
  String get bleStatusLocation;

  /// No description provided for @bleStatusUnsupported.
  ///
  /// In en, this message translates to:
  /// **'This phone has no Bluetooth LE'**
  String get bleStatusUnsupported;

  /// No description provided for @bleTurnOn.
  ///
  /// In en, this message translates to:
  /// **'Turn on'**
  String get bleTurnOn;

  /// No description provided for @bleOpenSettings.
  ///
  /// In en, this message translates to:
  /// **'Open settings'**
  String get bleOpenSettings;

  /// No description provided for @bleBackgroundNote.
  ///
  /// In en, this message translates to:
  /// **'In the background, only while \"Stay reachable in the background\" is on.'**
  String get bleBackgroundNote;

  /// No description provided for @reachableNotificationBle.
  ///
  /// In en, this message translates to:
  /// **'Reachable over Wi-Fi and Bluetooth'**
  String get reachableNotificationBle;

  /// No description provided for @newGroup.
  ///
  /// In en, this message translates to:
  /// **'New group'**
  String get newGroup;

  /// No description provided for @groupName.
  ///
  /// In en, this message translates to:
  /// **'Group name'**
  String get groupName;

  /// No description provided for @groupMembersPick.
  ///
  /// In en, this message translates to:
  /// **'Who to invite (up to 15 contacts)'**
  String get groupMembersPick;

  /// No description provided for @groupCreate.
  ///
  /// In en, this message translates to:
  /// **'Create'**
  String get groupCreate;

  /// No description provided for @groupCreateHelp.
  ///
  /// In en, this message translates to:
  /// **'You will be the group\'s admin: only you add or remove members. Everyone you invite sees the names and codes of the members who join.'**
  String get groupCreateHelp;

  /// No description provided for @groupMembersCount.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 member} other{{count} members}}'**
  String groupMembersCount(int count);

  /// No description provided for @groupJoining.
  ///
  /// In en, this message translates to:
  /// **'Joining… waiting for the admin'**
  String get groupJoining;

  /// No description provided for @groupInvitedBanner.
  ///
  /// In en, this message translates to:
  /// **'You are invited to this group.'**
  String get groupInvitedBanner;

  /// No description provided for @groupLeftBanner.
  ///
  /// In en, this message translates to:
  /// **'You left this group.'**
  String get groupLeftBanner;

  /// No description provided for @groupRemovedBanner.
  ///
  /// In en, this message translates to:
  /// **'The admin removed you from this group.'**
  String get groupRemovedBanner;

  /// No description provided for @groupJoiningBanner.
  ///
  /// In en, this message translates to:
  /// **'Waiting for the admin to add you. Messages can be sent once you are in.'**
  String get groupJoiningBanner;

  /// No description provided for @groupJoin.
  ///
  /// In en, this message translates to:
  /// **'Join'**
  String get groupJoin;

  /// No description provided for @groupDecline.
  ///
  /// In en, this message translates to:
  /// **'Decline'**
  String get groupDecline;

  /// No description provided for @groupInvitationsTitle.
  ///
  /// In en, this message translates to:
  /// **'Group invitations'**
  String get groupInvitationsTitle;

  /// No description provided for @groupInvitedBy.
  ///
  /// In en, this message translates to:
  /// **'Invited by {name}'**
  String groupInvitedBy(String name);

  /// No description provided for @groupInvitationHelp.
  ///
  /// In en, this message translates to:
  /// **'If you join, every member sees your name and MEM code, and you see theirs.'**
  String get groupInvitationHelp;

  /// No description provided for @groupInfo.
  ///
  /// In en, this message translates to:
  /// **'Group info'**
  String get groupInfo;

  /// No description provided for @groupAdmin.
  ///
  /// In en, this message translates to:
  /// **'admin'**
  String get groupAdmin;

  /// No description provided for @groupInvitedRole.
  ///
  /// In en, this message translates to:
  /// **'invited'**
  String get groupInvitedRole;

  /// No description provided for @groupYou.
  ///
  /// In en, this message translates to:
  /// **'you'**
  String get groupYou;

  /// No description provided for @groupNotAContact.
  ///
  /// In en, this message translates to:
  /// **'not your contact'**
  String get groupNotAContact;

  /// No description provided for @groupAddAsContact.
  ///
  /// In en, this message translates to:
  /// **'Add to contacts'**
  String get groupAddAsContact;

  /// No description provided for @groupAddAsContactHelp.
  ///
  /// In en, this message translates to:
  /// **'{name} becomes your contact. You can write to each other directly once they add you too.'**
  String groupAddAsContactHelp(String name);

  /// No description provided for @groupInvite.
  ///
  /// In en, this message translates to:
  /// **'Invite a contact'**
  String get groupInvite;

  /// No description provided for @groupRemove.
  ///
  /// In en, this message translates to:
  /// **'Remove from group'**
  String get groupRemove;

  /// No description provided for @groupLeave.
  ///
  /// In en, this message translates to:
  /// **'Leave group'**
  String get groupLeave;

  /// No description provided for @groupEnd.
  ///
  /// In en, this message translates to:
  /// **'End group for everyone'**
  String get groupEnd;

  /// No description provided for @groupLeaveConfirm.
  ///
  /// In en, this message translates to:
  /// **'Leave this group? You will not receive its messages any more.'**
  String get groupLeaveConfirm;

  /// No description provided for @groupEndConfirm.
  ///
  /// In en, this message translates to:
  /// **'End this group? Every member is removed.'**
  String get groupEndConfirm;

  /// No description provided for @deliveredToCount.
  ///
  /// In en, this message translates to:
  /// **'delivered to {done} of {total}'**
  String deliveredToCount(int done, int total);

  /// No description provided for @groupSentAutomatically.
  ///
  /// In en, this message translates to:
  /// **'Sent to each member automatically when a way to them opens'**
  String get groupSentAutomatically;

  /// No description provided for @groupsSection.
  ///
  /// In en, this message translates to:
  /// **'Groups'**
  String get groupsSection;

  /// No description provided for @contactsSection.
  ///
  /// In en, this message translates to:
  /// **'Contacts'**
  String get contactsSection;

  /// No description provided for @reason_unknown_group.
  ///
  /// In en, this message translates to:
  /// **'That group is not available.'**
  String get reason_unknown_group;

  /// No description provided for @reason_not_group_admin.
  ///
  /// In en, this message translates to:
  /// **'Only the group\'s admin can do that.'**
  String get reason_not_group_admin;

  /// No description provided for @reason_group_full.
  ///
  /// In en, this message translates to:
  /// **'A group has at most 16 members (and you can keep at most 64 groups).'**
  String get reason_group_full;

  /// No description provided for @reason_group_empty.
  ///
  /// In en, this message translates to:
  /// **'Nobody else is in this group yet.'**
  String get reason_group_empty;

  /// No description provided for @reason_not_a_contact.
  ///
  /// In en, this message translates to:
  /// **'This person is only a fellow group member, not your contact.'**
  String get reason_not_a_contact;

  /// No description provided for @channelHub.
  ///
  /// In en, this message translates to:
  /// **'Hub'**
  String get channelHub;

  /// No description provided for @hubToggle.
  ///
  /// In en, this message translates to:
  /// **'Deliver through a MEM hub'**
  String get hubToggle;

  /// No description provided for @hubToggleHelp.
  ///
  /// In en, this message translates to:
  /// **'Messages wait in a mailbox on the Internet until your contact collects them, so phones far apart still exchange messages. The hub\'s operator sees your IP address, when you connect, message sizes and short parts of your mailbox addresses, never the content. Anyone watching your network can tell you use MEM.'**
  String get hubToggleHelp;

  /// No description provided for @hubUrl.
  ///
  /// In en, this message translates to:
  /// **'Hub address'**
  String get hubUrl;

  /// No description provided for @hubUrlHint.
  ///
  /// In en, this message translates to:
  /// **'wss://hub.example.org/v1'**
  String get hubUrlHint;

  /// No description provided for @hubUrlSave.
  ///
  /// In en, this message translates to:
  /// **'Save'**
  String get hubUrlSave;

  /// No description provided for @hubNeedsUrl.
  ///
  /// In en, this message translates to:
  /// **'Enter the address of a hub first. There is no default hub yet.'**
  String get hubNeedsUrl;

  /// No description provided for @hubOnline.
  ///
  /// In en, this message translates to:
  /// **'Connected to the hub'**
  String get hubOnline;

  /// No description provided for @hubOffline.
  ///
  /// In en, this message translates to:
  /// **'Not connected; retrying'**
  String get hubOffline;

  /// No description provided for @devices.
  ///
  /// In en, this message translates to:
  /// **'Devices'**
  String get devices;

  /// No description provided for @devicesHelp.
  ///
  /// In en, this message translates to:
  /// **'Your MEM can be on up to four devices. The main device holds your identity: only it adds contacts and devices. People who carry your mail cannot tell the devices apart, but may notice that one message goes out several times at once.'**
  String get devicesHelp;

  /// No description provided for @thisDevice.
  ///
  /// In en, this message translates to:
  /// **'This device'**
  String get thisDevice;

  /// No description provided for @mainDevice.
  ///
  /// In en, this message translates to:
  /// **'Main device'**
  String get mainDevice;

  /// No description provided for @otherDevice.
  ///
  /// In en, this message translates to:
  /// **'Other device'**
  String get otherDevice;

  /// No description provided for @deviceWaiting.
  ///
  /// In en, this message translates to:
  /// **'Not joined yet. If adding it didn\'t finish, remove it and add the device again.'**
  String get deviceWaiting;

  /// No description provided for @addDevice.
  ///
  /// In en, this message translates to:
  /// **'Add a device'**
  String get addDevice;

  /// No description provided for @addDeviceHelp.
  ///
  /// In en, this message translates to:
  /// **'On the new device, choose “Use as another device” on the start screen. Then scan the code it shows.'**
  String get addDeviceHelp;

  /// No description provided for @addDeviceConfirmTitle.
  ///
  /// In en, this message translates to:
  /// **'Add this device?'**
  String get addDeviceConfirmTitle;

  /// No description provided for @addDeviceConfirmBody.
  ///
  /// In en, this message translates to:
  /// **'It will get all your contacts and receive your messages. Add only a device of your own, here with you.'**
  String get addDeviceConfirmBody;

  /// No description provided for @addDeviceShowSetup.
  ///
  /// In en, this message translates to:
  /// **'Show this to the new device'**
  String get addDeviceShowSetup;

  /// No description provided for @addDeviceSetupHint.
  ///
  /// In en, this message translates to:
  /// **'Hold the new device\'s camera to this code until it says it is ready.'**
  String get addDeviceSetupHint;

  /// No description provided for @removeDevice.
  ///
  /// In en, this message translates to:
  /// **'Remove'**
  String get removeDevice;

  /// No description provided for @removeDeviceTitle.
  ///
  /// In en, this message translates to:
  /// **'Remove this device?'**
  String get removeDeviceTitle;

  /// No description provided for @removeDeviceBody.
  ///
  /// In en, this message translates to:
  /// **'It stops receiving your messages, and deletes its copy of your MEM the next time it is online.'**
  String get removeDeviceBody;

  /// No description provided for @removeWaitingDeviceBody.
  ///
  /// In en, this message translates to:
  /// **'It never joined, so it has none of your messages. If it still reads the setup code, it will learn that it was removed.'**
  String get removeWaitingDeviceBody;

  /// No description provided for @joinAsDevice.
  ///
  /// In en, this message translates to:
  /// **'Use as another device'**
  String get joinAsDevice;

  /// No description provided for @joinTitle.
  ///
  /// In en, this message translates to:
  /// **'Another device of my MEM'**
  String get joinTitle;

  /// No description provided for @joinHelp.
  ///
  /// In en, this message translates to:
  /// **'On your main device, open ⋮ → Devices → Add a device and scan this code. Then point this camera at the code your main device shows.'**
  String get joinHelp;

  /// No description provided for @joinDone.
  ///
  /// In en, this message translates to:
  /// **'This device is now part of your MEM.'**
  String get joinDone;

  /// No description provided for @deviceRetiredTitle.
  ///
  /// In en, this message translates to:
  /// **'This device was removed'**
  String get deviceRetiredTitle;

  /// No description provided for @deviceRetiredBody.
  ///
  /// In en, this message translates to:
  /// **'Your main device removed it from your MEM. Its copy will be deleted now.'**
  String get deviceRetiredBody;

  /// No description provided for @addContactOnMain.
  ///
  /// In en, this message translates to:
  /// **'New contacts are added on your main device; they appear here by themselves.'**
  String get addContactOnMain;

  /// No description provided for @reason_main_device_only.
  ///
  /// In en, this message translates to:
  /// **'Only your main device can do this.'**
  String get reason_main_device_only;

  /// No description provided for @reason_too_many_devices.
  ///
  /// In en, this message translates to:
  /// **'Your MEM already has four devices.'**
  String get reason_too_many_devices;

  /// No description provided for @reason_device_known.
  ///
  /// In en, this message translates to:
  /// **'That device is already one of yours.'**
  String get reason_device_known;

  /// No description provided for @reason_unknown_device.
  ///
  /// In en, this message translates to:
  /// **'That is not one of your devices.'**
  String get reason_unknown_device;

  /// No description provided for @reason_not_empty.
  ///
  /// In en, this message translates to:
  /// **'This device already has contacts. Only a new installation can become another device.'**
  String get reason_not_empty;

  /// No description provided for @reason_not_for_this_device.
  ///
  /// In en, this message translates to:
  /// **'That code is for another device.'**
  String get reason_not_for_this_device;

  /// No description provided for @joinWrongSetup.
  ///
  /// In en, this message translates to:
  /// **'This code was made for an earlier request, so it can\'t be opened here. On the main device, remove the device marked “Not joined yet”, then add this device again by scanning the code above.'**
  String get joinWrongSetup;

  /// No description provided for @channelHubPartners.
  ///
  /// In en, this message translates to:
  /// **'Hubs of several operators'**
  String get channelHubPartners;

  /// No description provided for @hubPartners.
  ///
  /// In en, this message translates to:
  /// **'This hub also exchanges messages with the hubs of: {names}. A message to a contact who uses one of them is carried by both operators; both see what a hub sees. Such messages are marked P3.'**
  String hubPartners(String names);

  /// No description provided for @ownOperatorOnly.
  ///
  /// In en, this message translates to:
  /// **'Only my operator\'s hubs'**
  String get ownOperatorOnly;

  /// No description provided for @ownOperatorOnlyHelp.
  ///
  /// In en, this message translates to:
  /// **'Your messages never go to other operators\' hubs. Contacts who use another operator\'s hub then cannot get them through the hub.'**
  String get ownOperatorOnlyHelp;

  /// No description provided for @reason_invalid_hub_url.
  ///
  /// In en, this message translates to:
  /// **'Use a wss:// address (ws:// only for a hub on this device).'**
  String get reason_invalid_hub_url;

  /// No description provided for @pushNotification.
  ///
  /// In en, this message translates to:
  /// **'New messages'**
  String get pushNotification;

  /// No description provided for @pushToggle.
  ///
  /// In en, this message translates to:
  /// **'Wake up for new messages'**
  String get pushToggle;

  /// No description provided for @pushToggleHelp.
  ///
  /// In en, this message translates to:
  /// **'A UnifiedPush app on this phone (for example ntfy) wakes MEM when the hub has something for you, so messages arrive with MEM closed. The push service learns when MEM is woken, nothing else. Needs the hub.'**
  String get pushToggleHelp;

  /// No description provided for @pushNoDistributor.
  ///
  /// In en, this message translates to:
  /// **'Install a UnifiedPush app (for example ntfy) first.'**
  String get pushNoDistributor;

  /// No description provided for @pushPick.
  ///
  /// In en, this message translates to:
  /// **'Which push app?'**
  String get pushPick;

  /// No description provided for @channelPlain.
  ///
  /// In en, this message translates to:
  /// **'Plain text'**
  String get channelPlain;

  /// No description provided for @plainTitle.
  ///
  /// In en, this message translates to:
  /// **'Send as plain text?'**
  String get plainTitle;

  /// No description provided for @plainWarning.
  ///
  /// In en, this message translates to:
  /// **'This exact text leaves MEM unencrypted. The app you pick, its operator, and anyone who can read that chat will see it. It cannot be taken back.'**
  String get plainWarning;

  /// No description provided for @plainSend.
  ///
  /// In en, this message translates to:
  /// **'Send as plain text'**
  String get plainSend;

  /// No description provided for @reason_not_plain_text.
  ///
  /// In en, this message translates to:
  /// **'Only a one-to-one text message can be sent as plain text.'**
  String get reason_not_plain_text;

  /// No description provided for @channelEmail.
  ///
  /// In en, this message translates to:
  /// **'Email'**
  String get channelEmail;

  /// No description provided for @emailToggle.
  ///
  /// In en, this message translates to:
  /// **'Deliver by email'**
  String get emailToggle;

  /// No description provided for @emailToggleHelp.
  ///
  /// In en, this message translates to:
  /// **'MEM sends sealed messages from your mail account and collects MEM mails from it. Your mail provider sees both addresses, when, how much, and that it is MEM mail, never the content. Below your default privacy floor, so each send asks unless you allow email for a contact.'**
  String get emailToggleHelp;

  /// No description provided for @emailAddress.
  ///
  /// In en, this message translates to:
  /// **'Your email address'**
  String get emailAddress;

  /// No description provided for @emailImapHost.
  ///
  /// In en, this message translates to:
  /// **'IMAP server'**
  String get emailImapHost;

  /// No description provided for @emailSmtpHost.
  ///
  /// In en, this message translates to:
  /// **'SMTP server'**
  String get emailSmtpHost;

  /// No description provided for @emailPort.
  ///
  /// In en, this message translates to:
  /// **'Port'**
  String get emailPort;

  /// No description provided for @emailUsername.
  ///
  /// In en, this message translates to:
  /// **'Login'**
  String get emailUsername;

  /// No description provided for @emailPassword.
  ///
  /// In en, this message translates to:
  /// **'App password'**
  String get emailPassword;

  /// No description provided for @emailPasswordKept.
  ///
  /// In en, this message translates to:
  /// **'App password (saved; leave empty to keep)'**
  String get emailPasswordKept;

  /// No description provided for @emailSaveAccount.
  ///
  /// In en, this message translates to:
  /// **'Save account'**
  String get emailSaveAccount;

  /// No description provided for @emailRemoveAccount.
  ///
  /// In en, this message translates to:
  /// **'Remove account'**
  String get emailRemoveAccount;

  /// No description provided for @emailOnline.
  ///
  /// In en, this message translates to:
  /// **'Connected to your mailbox'**
  String get emailOnline;

  /// No description provided for @emailOffline.
  ///
  /// In en, this message translates to:
  /// **'Not connected; retrying'**
  String get emailOffline;

  /// No description provided for @emailSaved.
  ///
  /// In en, this message translates to:
  /// **'Mail account saved'**
  String get emailSaved;

  /// No description provided for @contactEmail.
  ///
  /// In en, this message translates to:
  /// **'Email address'**
  String get contactEmail;

  /// No description provided for @contactEmailHelp.
  ///
  /// In en, this message translates to:
  /// **'Only on this phone; MEM mails sealed messages here when you allow email for this contact.'**
  String get contactEmailHelp;

  /// No description provided for @contactEmailSave.
  ///
  /// In en, this message translates to:
  /// **'Save'**
  String get contactEmailSave;

  /// No description provided for @contactAliasEdit.
  ///
  /// In en, this message translates to:
  /// **'Rename'**
  String get contactAliasEdit;

  /// No description provided for @contactAliasTitle.
  ///
  /// In en, this message translates to:
  /// **'Name for this contact'**
  String get contactAliasTitle;

  /// No description provided for @contactAliasHelp.
  ///
  /// In en, this message translates to:
  /// **'Only you see this name; your contact is not told. Leave it empty to show the name they chose.'**
  String get contactAliasHelp;

  /// No description provided for @contactTheirName.
  ///
  /// In en, this message translates to:
  /// **'Their own name: {name}'**
  String contactTheirName(String name);

  /// No description provided for @contactTheirNameNone.
  ///
  /// In en, this message translates to:
  /// **'They chose no name'**
  String get contactTheirNameNone;

  /// No description provided for @reason_invalid_email_account.
  ///
  /// In en, this message translates to:
  /// **'Check the mail account: address, servers, ports, login and app password are all needed.'**
  String get reason_invalid_email_account;

  /// No description provided for @reason_invalid_email_address.
  ///
  /// In en, this message translates to:
  /// **'That is not an email address.'**
  String get reason_invalid_email_address;

  /// No description provided for @alreadyContact.
  ///
  /// In en, this message translates to:
  /// **'Already in your contacts'**
  String get alreadyContact;

  /// No description provided for @compareSymbols.
  ///
  /// In en, this message translates to:
  /// **'Compare symbols'**
  String get compareSymbols;

  /// No description provided for @channelWifiDirect.
  ///
  /// In en, this message translates to:
  /// **'Wi-Fi Direct'**
  String get channelWifiDirect;

  /// No description provided for @wifiDirectToggle.
  ///
  /// In en, this message translates to:
  /// **'Deliver over Wi-Fi Direct'**
  String get wifiDirectToggle;

  /// No description provided for @wifiDirectToggleHelp.
  ///
  /// In en, this message translates to:
  /// **'Phones nearby exchange messages over Wi-Fi without a router or the Internet. While on, nearby phones can see that yours runs MEM, and Android shows your phone\'s Wi-Fi Direct name to them. On Android 9 and older the other phone is asked to accept each connection. Android 12 and older need Location on to find phones; MEM never reads your location.'**
  String get wifiDirectToggleHelp;

  /// No description provided for @wifiDirectUnsupported.
  ///
  /// In en, this message translates to:
  /// **'This phone has no Wi-Fi Direct.'**
  String get wifiDirectUnsupported;

  /// No description provided for @inboundCardTitle.
  ///
  /// In en, this message translates to:
  /// **'Add this contact?'**
  String get inboundCardTitle;

  /// No description provided for @inboundCardBody.
  ///
  /// In en, this message translates to:
  /// **'Another app handed MEM a contact code. Add it only if you meant to: this person will be able to message you. Check the fingerprint with them if you can.'**
  String get inboundCardBody;

  /// No description provided for @wifiDirectPermission.
  ///
  /// In en, this message translates to:
  /// **'Permission to find nearby phones is missing'**
  String get wifiDirectPermission;

  /// No description provided for @wifiDirectLocation.
  ///
  /// In en, this message translates to:
  /// **'Location is off: this Android version needs it to find phones over Wi-Fi Direct'**
  String get wifiDirectLocation;

  /// No description provided for @removeContact.
  ///
  /// In en, this message translates to:
  /// **'Remove contact'**
  String get removeContact;

  /// No description provided for @removeContactTitle.
  ///
  /// In en, this message translates to:
  /// **'Remove this contact?'**
  String get removeContactTitle;

  /// No description provided for @removeContactBody.
  ///
  /// In en, this message translates to:
  /// **'Your conversation and any messages still waiting to be sent are deleted, and their messages will no longer reach you. Groups you share keep working. To talk again you need to pair again.'**
  String get removeContactBody;

  /// No description provided for @ruleHowLong.
  ///
  /// In en, this message translates to:
  /// **'For how long?'**
  String get ruleHowLong;

  /// No description provided for @ruleDays1.
  ///
  /// In en, this message translates to:
  /// **'1 day'**
  String get ruleDays1;

  /// No description provided for @ruleDays7.
  ///
  /// In en, this message translates to:
  /// **'7 days'**
  String get ruleDays7;

  /// No description provided for @ruleDays30.
  ///
  /// In en, this message translates to:
  /// **'30 days'**
  String get ruleDays30;

  /// No description provided for @ruleUntilRevoked.
  ///
  /// In en, this message translates to:
  /// **'Until I revoke'**
  String get ruleUntilRevoked;

  /// No description provided for @ruleUntil.
  ///
  /// In en, this message translates to:
  /// **'until {date}'**
  String ruleUntil(String date);

  /// No description provided for @soundPairButton.
  ///
  /// In en, this message translates to:
  /// **'Pair by sound'**
  String get soundPairButton;

  /// No description provided for @soundPairTitle.
  ///
  /// In en, this message translates to:
  /// **'Pair by sound'**
  String get soundPairTitle;

  /// No description provided for @soundPairHint.
  ///
  /// In en, this message translates to:
  /// **'Hold the phones close. On one phone tap Listen, on the other Play my code. The phones then take turns by themselves; at the end both show the same symbols. Compare them and tap Connect on both phones.'**
  String get soundPairHint;

  /// No description provided for @soundPairProgress.
  ///
  /// In en, this message translates to:
  /// **'Heard {received} of {needed} parts'**
  String soundPairProgress(int received, int needed);

  /// No description provided for @soundListen.
  ///
  /// In en, this message translates to:
  /// **'Listen'**
  String get soundListen;

  /// No description provided for @soundPlay.
  ///
  /// In en, this message translates to:
  /// **'Play my code'**
  String get soundPlay;

  /// No description provided for @soundStop.
  ///
  /// In en, this message translates to:
  /// **'Stop'**
  String get soundStop;

  /// No description provided for @soundIdle.
  ///
  /// In en, this message translates to:
  /// **'Choose what this phone does: listen, or play its code.'**
  String get soundIdle;

  /// No description provided for @soundListening.
  ///
  /// In en, this message translates to:
  /// **'Listening for the other phone…'**
  String get soundListening;

  /// No description provided for @soundPlaying.
  ///
  /// In en, this message translates to:
  /// **'Playing my code, round {round} of {rounds}…'**
  String soundPlaying(int round, int rounds);

  /// No description provided for @soundListenTimeout.
  ///
  /// In en, this message translates to:
  /// **'Nothing heard for two minutes. Bring the phones closer or make the room quieter, then listen again.'**
  String get soundListenTimeout;

  /// No description provided for @soundPlayDone.
  ///
  /// In en, this message translates to:
  /// **'My code was played three times. If the other phone did not get it, play it again.'**
  String get soundPlayDone;

  /// No description provided for @soundTurnWait.
  ///
  /// In en, this message translates to:
  /// **'Their code arrived. Waiting for a quiet moment to play mine…'**
  String get soundTurnWait;

  /// No description provided for @soundNoAnswer.
  ///
  /// In en, this message translates to:
  /// **'The other phone did not answer. Bring the phones closer or make the room quieter, then start again.'**
  String get soundNoAnswer;

  /// No description provided for @soundDecline.
  ///
  /// In en, this message translates to:
  /// **'Decline'**
  String get soundDecline;

  /// No description provided for @soundConnect.
  ///
  /// In en, this message translates to:
  /// **'Connect'**
  String get soundConnect;

  /// No description provided for @soundPaired.
  ///
  /// In en, this message translates to:
  /// **'Connected. You can write to each other now.'**
  String get soundPaired;

  /// No description provided for @soundDeclined.
  ///
  /// In en, this message translates to:
  /// **'Declined. A contact this exchange added was removed.'**
  String get soundDeclined;

  /// No description provided for @soundDeclinedKept.
  ///
  /// In en, this message translates to:
  /// **'Declined. The contact you already had stays as it was.'**
  String get soundDeclinedKept;

  /// No description provided for @soundPairGot.
  ///
  /// In en, this message translates to:
  /// **'Their code arrived.'**
  String get soundPairGot;

  /// No description provided for @soundPairNoMicrophone.
  ///
  /// In en, this message translates to:
  /// **'Pairing by sound needs the microphone. Allow it in the phone\'s settings, or pair with the QR code.'**
  String get soundPairNoMicrophone;

  /// No description provided for @soundPairFailed.
  ///
  /// In en, this message translates to:
  /// **'Sound could not start on this phone. Pair with the QR code instead.'**
  String get soundPairFailed;

  /// No description provided for @soundToneTitle.
  ///
  /// In en, this message translates to:
  /// **'Sound'**
  String get soundToneTitle;

  /// No description provided for @soundToneAudible.
  ///
  /// In en, this message translates to:
  /// **'Audible'**
  String get soundToneAudible;

  /// No description provided for @soundToneInaudible.
  ///
  /// In en, this message translates to:
  /// **'Almost inaudible'**
  String get soundToneInaudible;

  /// No description provided for @soundPairExposure.
  ///
  /// In en, this message translates to:
  /// **'Anyone within earshot, or any recording, can pick up your code, just as anyone who sees your QR code can. Messages never travel by sound. Both phones must use the same sound.'**
  String get soundPairExposure;

  /// No description provided for @channelAudio.
  ///
  /// In en, this message translates to:
  /// **'Sound'**
  String get channelAudio;

  /// No description provided for @nearbyTitle.
  ///
  /// In en, this message translates to:
  /// **'Nearby'**
  String get nearbyTitle;

  /// No description provided for @connectBySound.
  ///
  /// In en, this message translates to:
  /// **'Connect by sound'**
  String get connectBySound;

  /// No description provided for @connectByCode.
  ///
  /// In en, this message translates to:
  /// **'Connect by code'**
  String get connectByCode;

  /// No description provided for @nearbyHelp.
  ///
  /// In en, this message translates to:
  /// **'Phones on the same Wi-Fi usually find each other by themselves. Some networks (guest, hotel, office) block that: then connect by sound or by code. One phone tells the other where to reach it on this Wi-Fi; the messages themselves go over Wi-Fi.'**
  String get nearbyHelp;

  /// No description provided for @connectSoundHint.
  ///
  /// In en, this message translates to:
  /// **'Hold the phones close. On one phone tap Listen, on the other Play my code. The listening phone connects to the playing one over this Wi-Fi; one code takes about 6 seconds.'**
  String get connectSoundHint;

  /// No description provided for @connectSoundExposure.
  ///
  /// In en, this message translates to:
  /// **'People nearby, or a recording, can hear that a MEM phone is here, its address on this network and a temporary key that changes when MEM restarts. Nothing that identifies you, and no messages.'**
  String get connectSoundExposure;

  /// No description provided for @connectGot.
  ///
  /// In en, this message translates to:
  /// **'Connecting to the other phone. Messages for your contacts follow by themselves.'**
  String get connectGot;

  /// No description provided for @connectNoMicrophone.
  ///
  /// In en, this message translates to:
  /// **'Connecting by sound needs the microphone. Allow it in the phone\'s settings, or connect by code.'**
  String get connectNoMicrophone;

  /// No description provided for @connectFailed.
  ///
  /// In en, this message translates to:
  /// **'Sound could not start on this phone. Connect by code instead.'**
  String get connectFailed;

  /// No description provided for @connectCodeHint.
  ///
  /// In en, this message translates to:
  /// **'Let the other phone scan this code, or scan theirs below.'**
  String get connectCodeHint;

  /// No description provided for @reason_no_local_network.
  ///
  /// In en, this message translates to:
  /// **'Turn on Wi-Fi and the local network channel (Channels) first.'**
  String get reason_no_local_network;

  /// No description provided for @attestTitle.
  ///
  /// In en, this message translates to:
  /// **'Confirm in person'**
  String get attestTitle;

  /// No description provided for @attestScanRequest.
  ///
  /// In en, this message translates to:
  /// **'Scan the code on the service\'s page, or paste it. You confirm together with the other person, each on your own phone.'**
  String get attestScanRequest;

  /// No description provided for @attestMainOnly.
  ///
  /// In en, this message translates to:
  /// **'Only your main device can confirm in person.'**
  String get attestMainOnly;

  /// No description provided for @attestAskedBy.
  ///
  /// In en, this message translates to:
  /// **'Asked by'**
  String get attestAskedBy;

  /// No description provided for @attestWhat.
  ///
  /// In en, this message translates to:
  /// **'What'**
  String get attestWhat;

  /// No description provided for @attestRole.
  ///
  /// In en, this message translates to:
  /// **'Your role'**
  String get attestRole;

  /// No description provided for @attestCheckCode.
  ///
  /// In en, this message translates to:
  /// **'Check that the page shows the same code: {code}'**
  String attestCheckCode(String code);

  /// No description provided for @attestExchangeHint.
  ///
  /// In en, this message translates to:
  /// **'Show your code to the other phone, then scan theirs.'**
  String get attestExchangeHint;

  /// No description provided for @attestCompare.
  ///
  /// In en, this message translates to:
  /// **'Compare these symbols with the ones on the other phone. Confirm only if they match and the other person is in front of you.'**
  String get attestCompare;

  /// No description provided for @attestSending.
  ///
  /// In en, this message translates to:
  /// **'Sending…'**
  String get attestSending;

  /// No description provided for @attestSent.
  ///
  /// In en, this message translates to:
  /// **'Sent. {origin} now has your confirmation.'**
  String attestSent(String origin);

  /// No description provided for @attestSendFailed.
  ///
  /// In en, this message translates to:
  /// **'It could not be sent. Copy this code and paste it on the service\'s page.'**
  String get attestSendFailed;

  /// No description provided for @attestTryAgain.
  ///
  /// In en, this message translates to:
  /// **'Try again'**
  String get attestTryAgain;

  /// No description provided for @attestCancelled.
  ///
  /// In en, this message translates to:
  /// **'Cancelled. Nothing was signed.'**
  String get attestCancelled;

  /// No description provided for @attestErrWrongObject.
  ///
  /// In en, this message translates to:
  /// **'This is not the code needed here.'**
  String get attestErrWrongObject;

  /// No description provided for @attestErrInvalid.
  ///
  /// In en, this message translates to:
  /// **'This code is damaged or not allowed.'**
  String get attestErrInvalid;

  /// No description provided for @attestErrOtherSubject.
  ///
  /// In en, this message translates to:
  /// **'The other phone is confirming something else.'**
  String get attestErrOtherSubject;

  /// No description provided for @attestErrSameRole.
  ///
  /// In en, this message translates to:
  /// **'Both phones have the same role. Each person must scan their own code on the service\'s page.'**
  String get attestErrSameRole;

  /// No description provided for @attestErrOwnCode.
  ///
  /// In en, this message translates to:
  /// **'This is your own code. Scan the other phone.'**
  String get attestErrOwnCode;

  /// No description provided for @attestLog.
  ///
  /// In en, this message translates to:
  /// **'Confirmed in person'**
  String get attestLog;

  /// No description provided for @attestLogEntry.
  ///
  /// In en, this message translates to:
  /// **'{origin} · {how}'**
  String attestLogEntry(String origin, String how);

  /// No description provided for @attestLogSent.
  ///
  /// In en, this message translates to:
  /// **'sent'**
  String get attestLogSent;

  /// No description provided for @attestLogCopied.
  ///
  /// In en, this message translates to:
  /// **'copied'**
  String get attestLogCopied;
}

class _AppLocalizationsDelegate
    extends LocalizationsDelegate<AppLocalizations> {
  const _AppLocalizationsDelegate();

  @override
  Future<AppLocalizations> load(Locale locale) {
    return SynchronousFuture<AppLocalizations>(lookupAppLocalizations(locale));
  }

  @override
  bool isSupported(Locale locale) =>
      <String>['en', 'uk'].contains(locale.languageCode);

  @override
  bool shouldReload(_AppLocalizationsDelegate old) => false;
}

AppLocalizations lookupAppLocalizations(Locale locale) {
  // Lookup logic when only language code is specified.
  switch (locale.languageCode) {
    case 'en':
      return AppLocalizationsEn();
    case 'uk':
      return AppLocalizationsUk();
  }

  throw FlutterError(
    'AppLocalizations.delegate failed to load unsupported locale "$locale". This is likely '
    'an issue with the localizations generation tool. Please file an issue '
    'on GitHub with a reproducible sample app and the gen-l10n configuration '
    'that was used.',
  );
}
