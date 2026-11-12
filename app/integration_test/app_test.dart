// End-to-end test of the real app: real Rust core, real encrypted storage, real UI, real
// local network.
//
// Device A is driven through the UI; device B is a second node in the same process driven
// through the API. The Wi-Fi part uses real mDNS and Iroh on this machine's network; set
// MEM_IT_SKIP_LAN=1 where multicast is unavailable.
// Screenshots of the app's own render tree are written to $MEM_SCREENSHOT_DIR if set.

import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:material_ui/material_ui.dart' as mui;
import 'package:mem/app_scope.dart';
import 'package:mem/main.dart';
import 'package:mem/src/rust/api.dart';
import 'package:mem/src/ui_settings.dart';
import 'package:mem/src/rust/frb_generated.dart';
import 'package:mem/widgets/qr.dart';

final _boundary = GlobalKey();

Future<void> _shot(
  WidgetTester tester,
  String name, {
  bool settle = true,
}) async {
  final dir = Platform.environment['MEM_SCREENSHOT_DIR'];
  if (dir == null) return;
  if (settle) await _calm(tester);
  await tester.runAsync(() async {
    final boundary =
        _boundary.currentContext!.findRenderObject()! as RenderRepaintBoundary;
    final image = await boundary.toImage(pixelRatio: 1.5);
    final png = await image.toByteData(format: ui.ImageByteFormat.png);
    await File('$dir/$name.png').writeAsBytes(png!.buffer.asUint8List());
  });
}

Future<void> _settle(WidgetTester tester) async {
  // FRB async calls complete outside the fake clock: give them real time.
  for (var i = 0; i < 10; i++) {
    await tester.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 50)),
    );
    await tester.pump(const Duration(milliseconds: 100));
  }
  await _calm(tester);
}

/// Waits until nothing animates, or, where something always does (the live camera preview on
/// Android: the Scan screens), pumps frames for a while and goes on.
Future<void> _calm(WidgetTester tester) async {
  try {
    await tester.pumpAndSettle(
      const Duration(milliseconds: 100),
      EnginePhase.sendSemanticsUpdate,
      const Duration(seconds: 4),
    );
  } on FlutterError catch (e) {
    if (!e.message.contains('pumpAndSettle timed out')) rethrow;
    for (var i = 0; i < 10; i++) {
      await tester.pump(const Duration(milliseconds: 100));
    }
  }
}

/// Waits in real time (FRB and the network run outside the fake clock) until [done].
Future<void> _until(
  WidgetTester tester,
  bool Function() done, {
  Duration timeout = const Duration(seconds: 40),
}) async {
  final end = DateTime.now().add(timeout);
  while (!done()) {
    if (DateTime.now().isAfter(end)) fail('timed out');
    await tester.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 200)),
    );
    await tester.pump();
  }
}

/// Waits for the channel sheet after a send. The offers come from the core in real time, so the
/// sheet may open a moment after the frames settle.
Future<void> _sheet(WidgetTester tester) async {
  await _until(
    tester,
    () => find.text('How should this message travel?').evaluate().isNotEmpty,
  );
  await _settle(tester);
}

/// Sends [text] from the chat composer and waits for the channel sheet. A note shown over the
/// composer can swallow the tap that connects the keyboard, and the send action is then lost:
/// the text stays in the field, so try again.
Future<void> _send(WidgetTester tester, String text) async {
  final field = find.byType(mui.TextField);
  for (var attempt = 0; attempt < 5; attempt++) {
    await tester.tap(field);
    await _settle(tester);
    await tester.enterText(field, text);
    await tester.testTextInput.receiveAction(TextInputAction.send);
    await _settle(tester);
    if (tester.widget<mui.TextField>(field).controller?.text != text) break;
  }
  await _sheet(tester);
}

/// Opens the home screen's menu and picks [label].
Future<void> _menu(WidgetTester tester, String label) async {
  await tester.tap(find.byIcon(Icons.more_vert));
  await _calm(tester);
  await tester.tap(find.text(label).last);
}

/// Types into the chat composer. After a route was pushed and popped the field keeps focus
/// but loses its input connection, so tap it to reconnect first.
Future<void> _type(WidgetTester tester, String text) async {
  final field = find.byType(mui.TextField);
  await tester.tap(field);
  await tester.pump();
  await tester.enterText(field, text);
  await tester.pump();
  expect(tester.widget<mui.TextField>(field).controller?.text ?? text, text);
}

/// A QR frame as a camera would see it: luma, 4 px per module, white quiet zone.
(Uint8List, int) _photograph(QrView qr) {
  const scale = 4;
  final side = (qr.width + 8) * scale;
  final img = Uint8List(side * side)..fillRange(0, side * side, 235);
  for (var y = 0; y < qr.width; y++) {
    for (var x = 0; x < qr.width; x++) {
      if (qr.modules[y * qr.width + x] != 1) continue;
      for (var dy = 0; dy < scale; dy++) {
        final row = ((y + 4) * scale + dy) * side + (x + 4) * scale;
        img.fillRange(row, row + scale, 20);
      }
    }
  }
  return (img, side);
}

/// A relying party's request as `memattreq1:` text: canonical CBOR by hand,
/// as a service would make it.
String _attestRequest({
  required String origin,
  required String role,
  required int challenge,
  required String submitUrl,
}) {
  final out = <int>[];
  void head(int major, int n) {
    if (n < 24) {
      out.add(major << 5 | n);
    } else if (n < 256) {
      out.addAll([major << 5 | 24, n]);
    } else {
      out.addAll([major << 5 | 25, n >> 8, n & 255]);
    }
  }

  void text(String s) {
    final b = s.codeUnits;
    head(3, b.length);
    out.addAll(b);
  }

  void bytes(List<int> b) {
    head(2, b.length);
    out.addAll(b);
  }

  head(5, 7);
  head(0, 0);
  head(0, 1);
  head(0, 1);
  text(origin);
  head(0, 2);
  bytes(List.filled(32, 7));
  head(0, 3);
  text('basa0:a4dqobyh');
  head(0, 4);
  text(role);
  head(0, 5);
  bytes(List.filled(16, challenge));
  head(0, 6);
  text(submitUrl);
  return 'memattreq1:${base64Url.encode(out).replaceAll('=', '')}';
}

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  // The Rust library can be loaded once per process, so the tests share it.
  setUpAll(RustLib.init);

  testWidgets('pair, consent-gated send, receive with provenance', (
    tester,
  ) async {
    // The rainbow outline (widgets/glow.dart) spins forever unless animations are disabled, which
    // would stall pumpAndSettle; it stops when the platform asks for reduced motion.
    tester.platformDispatcher.accessibilityFeaturesTestValue =
        const FakeAccessibilityFeatures(disableAnimations: true);
    addTearDown(tester.platformDispatcher.clearAccessibilityFeaturesTestValue);
    final tmp = await Directory.systemTemp.createTemp('mem-it-');
    final a = await openNode(dir: '${tmp.path}/a');
    final b = await openNode(dir: '${tmp.path}/b');
    await b.setDisplayName(name: 'Bohdan');
    // Start with the local network off, so each channel below is exercised on purpose.
    await a.setLanEnabled(enabled: false);
    await b.setLanEnabled(enabled: false);
    final state = AppState(a);
    // This test reads the full channel badges; they stay full instead of collapsing to symbols
    // after a delay (the collapsing itself has its own tests).
    state.settings.badgeMode = UiSettings.maxBadgeMode;

    await tester.pumpWidget(
      RepaintBoundary(
        key: _boundary,
        child: MemApp(state: state),
      ),
    );
    await _calm(tester);
    expect(find.text('No contacts yet'), findsOneWidget);
    await _shot(tester, '01-empty');

    // --- Pair: A pastes B's code (remote pairing), B imports A's.
    await tester.tap(find.byTooltip('Add contact'));
    await _calm(tester);
    await _shot(tester, '02-my-code');
    await tester.tap(find.text('Scan or paste'));
    await _calm(tester);
    await tester.enterText(find.byType(TextField), b.ownCard().text);
    await tester.tap(find.text('Open'));
    await _settle(tester);
    expect(find.text('Contact added'), findsOneWidget);
    await _shot(tester, '03-sas');
    await tester.tap(find.text('They match'));
    await _calm(tester);
    final paired = await b.receiveText(
      text: a.ownCard().text,
      channel: Channel.share,
    );
    expect(paired, isA<ReceiveOutcome_Contact>());
    // Both sides show the same SAS.
    final sasB = (paired as ReceiveOutcome_Contact).sas;
    expect(a.contacts().single.sas, sasB);
    await tester.pageBack();
    await _calm(tester);
    expect(find.text('Bohdan'), findsOneWidget);

    // --- Send: below-floor channel must ask first.
    await tester.tap(find.text('Bohdan'));
    await _calm(tester);
    // The chat composer comes from flutter_chat_ui, built on material_ui.
    await _send(tester, 'Привіт! Meet at 6?');
    expect(find.text('How should this message travel?'), findsOneWidget);
    await _shot(tester, '04-channels');
    await tester.tap(find.text('Sharing through another app'));
    await _settle(tester);
    expect(find.text('Privacy check'), findsOneWidget);
    expect(find.text('that you use MEM'), findsOneWidget);
    expect(
      find.text('the message text'),
      findsOneWidget,
    ); // listed under "Stays private"
    await _shot(tester, '05-consent');
    await tester.tap(find.text('Allow for this message'));
    await _settle(tester);
    expect(find.text('Send through another app'), findsOneWidget);
    final armored = tester
        .widget<SelectableText>(find.byType(SelectableText))
        .data!;
    expect(armored, startsWith('mem1:'));
    await _shot(tester, '06-share');
    await tester.tap(find.text('Copy'));
    await _settle(tester);

    // B receives the bundle that A's UI produced.
    final got = await b.receiveText(
      text: 'from A: $armored',
      channel: Channel.share,
    );
    expect(got, isA<ReceiveOutcome_Message>());
    final bContacts = b.contacts();
    expect(
      b.messages(contactId: bContacts.single.id).single.text,
      'Привіт! Meet at 6?',
    );

    // The privacy log recorded the downgrade.
    expect(a.privacyLog().single.grade, 'P4');

    // A standing rule with an end date: "Allow for this contact…",
    // 1 day. The next send by sharing then goes without asking, and is still logged.
    await tester.pump(
      const Duration(seconds: 6),
    ); // the "Copied" note covers the composer
    await _settle(tester);
    await _send(tester, 'second');
    await tester.tap(find.text('Sharing through another app'));
    await _settle(tester);
    await tester.tap(find.text('Allow for this contact…'));
    await _settle(tester);
    expect(find.text('For how long?'), findsOneWidget);
    await _shot(tester, '06b-rule-duration');
    await tester.tap(find.text('1 day'));
    await _settle(tester);
    await tester.tap(find.text('Allow'));
    await _settle(tester);
    expect(find.text('Send through another app'), findsOneWidget);
    await tester.tap(find.text('Copy'));
    await _settle(tester);
    final rule = a.rules().single;
    final left =
        rule.untilUnixMs!.toInt() - DateTime.now().millisecondsSinceEpoch;
    expect(left, inInclusiveRange(86_400_000 - 120_000, 86_400_000));
    await tester.pump(const Duration(seconds: 6));
    await _settle(tester);
    await _send(tester, 'third');
    await tester.tap(find.text('Sharing through another app'));
    await _settle(tester);
    expect(find.text('Privacy check'), findsNothing);
    expect(find.text('Send through another app'), findsOneWidget);
    await tester.tap(find.text('Copy'));
    await _settle(tester);
    expect(a.privacyLog().length, 3);

    // P5: plain text offers only "this message", then shows the exact
    // text on a second screen; backing out there sends nothing.
    await tester.pump(const Duration(seconds: 6));
    await _settle(tester);
    await _send(tester, 'plain words');
    await tester.ensureVisible(
      find.text('Plain text through another app (not encrypted)'),
    );
    await _settle(tester);
    await tester.tap(
      find.text('Plain text through another app (not encrypted)'),
    );
    await _settle(tester);
    expect(find.text('Privacy check'), findsOneWidget);
    expect(find.text('Allow for this contact…'), findsNothing);
    await _shot(tester, '06c-plain-consent');
    await tester.tap(find.text('Allow for this message'));
    await _settle(tester);
    expect(find.text('Send as plain text?'), findsOneWidget);
    expect(
      tester.widget<SelectableText>(find.byType(SelectableText)).data,
      'plain words',
    );
    await _shot(tester, '06d-plain-text');
    await tester.tap(find.text("Don't send"));
    await _settle(tester);
    expect(find.text('Send as plain text?'), findsNothing);
    // The log records the consent when the text left the core, even though it was not shared:
    // it may over-report an exposure, never hide one.
    expect(a.privacyLog().length, 4);
    expect(a.privacyLog().first.grade, 'P5');

    // --- Reply from B by animated QR (P0). A's scanner (rxing in Rust, via FFI) decodes
    // camera-like images of B's frames, skipping some, as a camera would.
    final reply = await b.sendText(
      contactId: bContacts.single.id,
      text: 'Yes, see you 👍',
    );
    expect(reply.sendingNearby, isFalse);
    expect(reply.sendingHub, isFalse);
    final ready = await b.transmit(
      messageId: reply.messageId,
      channel: Channel.qr,
      locale: 'en',
    );
    final animation = (ready as TransmitOutcome_Ready).animation!;
    final scanner = newQrScanner();
    ScanUpdate update = const ScanUpdate.nothing();
    // Post-quantum bundles carrying a key update are about 5 KB: ~25 frames.
    for (var n = 1; n < 160 && update is! ScanUpdate_Received; n += 2) {
      final (luma, side) = _photograph(animation.frame(n: n)!);
      update = await a.scanFrame(
        scanner: scanner,
        luma: luma,
        width: side,
        height: side,
        rowStride: side,
      );
    }
    expect(
      (update as ScanUpdate_Received).field0,
      isA<ReceiveOutcome_Message>(),
    );
    state.changed();
    await _settle(tester);
    expect(find.text('Yes, see you 👍'), findsOneWidget);
    expect(find.text('QR · P0'), findsWidgets);
    await _shot(tester, '07-chat');

    // --- A sends by animated QR from the UI.
    // Let the "Copied" snackbar from the share step expire; it covers the composer.
    await tester.pump(const Duration(seconds: 5));
    await _calm(tester);
    await _type(tester, 'On my way');
    await tester.tap(find.byIcon(mui.Icons.send));
    await _settle(tester);
    await tester.tap(find.text('QR code on screen'));
    for (var i = 0; i < 5; i++) {
      await tester.runAsync(
        () => Future<void>.delayed(const Duration(milliseconds: 100)),
      );
      await tester.pump(const Duration(milliseconds: 130));
    }
    expect(find.text('Show this to your contact'), findsOneWidget);
    await _shot(tester, '07b-animated-qr', settle: false);
    await tester.ensureVisible(find.text('Done'));
    await tester.pump();
    await tester.tap(find.text('Done'));
    await _settle(tester);

    // Pasted text arrived through another app: honest P4 provenance.
    await tester.pageBack();
    await _calm(tester);
    final pasteId = await b.sendText(
      contactId: bContacts.single.id,
      text: 'pasted',
    );
    final shared = await b.transmit(
      messageId: pasteId.messageId,
      channel: Channel.qr,
      locale: 'en',
    );
    await _menu(tester, 'Receive');
    await _calm(tester);
    await tester.enterText(
      find.byType(TextField),
      (shared as TransmitOutcome_Ready).text,
    );
    await tester.tap(find.text('Open'));
    await _settle(tester);
    expect(find.text('pasted'), findsOneWidget);
    expect(find.text('other app · P4'), findsWidgets);

    // --- Privacy log screen.
    await tester.pageBack();
    await _calm(tester);
    await _menu(tester, 'Privacy log');
    await _calm(tester);
    // Once, the new rule, then the rule again (and the P5 consent); the rule shows its end date.
    expect(find.textContaining('P4 below floor P3'), findsNWidgets(3));
    expect(find.textContaining('P5 below floor P3'), findsOneWidget);
    expect(find.textContaining('· until '), findsOneWidget);
    await _shot(tester, '08-privacy-log');
    // Revoking it here makes sharing ask again.
    await tester.tap(find.text('Revoke'));
    await _settle(tester);
    expect(a.rules(), isEmpty);

    // --- Ukrainian locale renders the consent text from the core in Ukrainian.
    final qid = await a.sendText(
      contactId: a.contacts().single.id,
      text: 'ще одне',
    );
    final uk = await a.transmit(
      messageId: qid.messageId,
      channel: Channel.share,
      locale: 'uk',
    );
    expect(
      (uk as TransmitOutcome_ConsentRequired).field0.title,
      contains('нижчий за поріг'),
    );

    if (Platform.environment['MEM_IT_SKIP_LAN'] != '1') {
      // --- Wi-Fi (P1): turn the channel on in the UI; the two nodes find each other by
      // mDNS and connect directly over Iroh.
      await tester.pageBack();
      await _calm(tester);
      await tester.tap(find.byTooltip('Wi-Fi delivery off'));
      await _calm(tester);
      // Nearby: connecting by sound or code needs the local network first.
      expect(find.text('Nearby'), findsOneWidget);
      expect(
        tester
            .widget<ButtonStyleButton>(
              find.ancestor(
                of: find.text('Connect by sound'),
                matching: find.byWidgetPredicate((w) => w is ButtonStyleButton),
              ),
            )
            .onPressed,
        isNull,
      );
      await tester.tap(find.text('Channels'));
      await _calm(tester);
      await _shot(tester, '09-channels');
      // Below the Bluetooth card: the list builds it once scrolled to.
      await tester.scrollUntilVisible(
        find.text('Wi-Fi: the same local network'),
        300,
      );
      expect(find.text('Wi-Fi: the same local network'), findsOneWidget);
      await tester.ensureVisible(find.text('Deliver over the local network'));
      await _calm(tester);
      await tester.tap(find.text('Deliver over the local network'));
      await _settle(tester);
      await b.setLanEnabled(enabled: true);
      final bId = a.contacts().single.id;
      await _until(tester, () => a.nearby().contains(bId));
      await tester.pageBack();
      await _settle(tester);
      expect(find.text('1 MEM device nearby'), findsOneWidget);
      // Connect by code: this phone's hint as one QR code (a single frame).
      final hint = await a.connectHintQr();
      expect(hint.framesNeeded(), 1);
      await _shot(tester, '10-nearby');
      await tester.pageBack();
      await _settle(tester);

      // The message A queued earlier without choosing a channel went out by itself.
      final aId = bContacts.single.id;
      await _until(
        tester,
        () => b.messages(contactId: aId).any((m) => m.text == 'ще одне'),
      );

      // B -> A over Wi-Fi, no user action on either side.
      final wifi = await b.sendText(contactId: aId, text: 'Over Wi-Fi 📶');
      expect(wifi.sendingNearby, isTrue);
      await _until(
        tester,
        () => a.messages(contactId: bId).any((m) => m.text == 'Over Wi-Fi 📶'),
      );
      await tester.tap(find.text('Bohdan'));
      await _settle(tester);
      expect(find.text('Over Wi-Fi 📶'), findsOneWidget);
      expect(find.text('Wi-Fi · P1'), findsWidgets);
      expect(find.text('nearby'), findsOneWidget);

      // A -> B from the UI: no channel sheet, it just goes.
      await _type(tester, 'Got it ✅');
      await tester.tap(find.byIcon(mui.Icons.send));
      await _settle(tester);
      expect(find.text('How should this message travel?'), findsNothing);
      await _until(
        tester,
        () => b.messages(contactId: aId).any((m) => m.text == 'Got it ✅'),
      );
      final got = b
          .messages(contactId: aId)
          .firstWhere((m) => m.text == 'Got it ✅');
      expect(got.channels.single.channel, Channel.lan);
      expect(got.channels.single.grade, 'P1');
      // Confirmed by B's ack: two ticks.
      await _until(
        tester,
        () => a
            .messages(contactId: bId)
            .any(
              (m) =>
                  m.text == 'Got it ✅' && m.status == MessageStatus.delivered,
            ),
      );
      await _settle(tester);
      await _shot(tester, '11-wifi-chat');
      await tester.pageBack();
      await _settle(tester);

      // --- A group, created in the UI; B joins through the API.
      await _menu(tester, 'New group');
      await _settle(tester);
      await tester.enterText(
        find.widgetWithText(TextField, 'Group name'),
        'Trail',
      );
      await tester.tap(find.widgetWithText(CheckboxListTile, 'Bohdan'));
      await tester.pump();
      await tester.tap(find.text('Create'));
      await _settle(tester);
      final groupId = a.groups().single.id;
      await _until(
        tester,
        () => b.groups().any((g) => g.status == GroupStatusItem.invited),
      );
      await b.answerGroupInvitation(groupId: groupId, accept: true);
      await _until(
        tester,
        () => b.groups().single.status == GroupStatusItem.member,
      );
      await _type(tester, 'Hello group 🥾');
      await tester.tap(find.byIcon(mui.Icons.send));
      await _settle(tester);
      await _until(
        tester,
        () => b
            .groupMessages(groupId: groupId)
            .any((m) => m.text == 'Hello group 🥾'),
      );
      await b.sendGroupText(groupId: groupId, text: 'On my way');
      await _until(
        tester,
        () =>
            a.groupMessages(groupId: groupId).any((m) => m.text == 'On my way'),
      );
      await _until(
        tester,
        () => a
            .groupMessages(groupId: groupId)
            .any((m) => m.outgoing && m.deliveredTo == 1),
      );
      await _settle(tester);
      expect(find.text('On my way'), findsOneWidget);
      expect(find.text('delivered to 1 of 1'), findsOneWidget);
      await _shot(tester, '11b-group-chat');
      await tester.pageBack();
      await _settle(tester);
      expect(find.text('Trail'), findsOneWidget);
    } else {
      await tester.pageBack();
      await _settle(tester);
    }

    // Backup and recovery, through the UI: turn on, see the phrase, check 3 words.
    await _menu(tester, 'Settings');
    await _settle(tester);
    await _shot(tester, '12a-settings');
    await tester.scrollUntilVisible(
      find.widgetWithText(ListTile, 'Backup and recovery'),
      300,
      scrollable: find.byType(Scrollable).first,
    );
    await _settle(tester);
    await tester.tap(find.widgetWithText(ListTile, 'Backup and recovery'));
    await _settle(tester);
    // Leaving the phrase without the 3-word check changes nothing.
    await tester.tap(find.text('Turn on backups'));
    await _settle(tester);
    await tester.pageBack();
    await _settle(tester);
    expect(a.backupStatus().enabled, isFalse);
    expect(
      find.text('The new phrase was not confirmed, so nothing changed.'),
      findsOneWidget,
    );
    await tester.pump(const Duration(seconds: 5));
    await _calm(tester);

    await tester.tap(find.text('Turn on backups'));
    await _settle(tester);
    final words = await a.backupPhraseShow();
    expect(words, hasLength(12));
    for (final w in words) {
      expect(find.textContaining(w), findsWidgets);
    }
    await _shot(tester, '12-phrase');
    await tester.tap(find.text('I have written it down'));
    await _settle(tester);
    for (var n = 1; n <= words.length; n++) {
      final field = find.widgetWithText(TextField, 'Word $n');
      if (field.evaluate().isNotEmpty) {
        await tester.enterText(field, words[n - 1]);
      }
    }
    await tester.tap(find.text('Done'));
    await _settle(tester);
    expect(find.text('Backups are on.'), findsOneWidget);
    expect(a.backupStatus().enabled, isTrue);

    // Settings: detailed badges and no message times, remembered and applied in the chat.
    await tester.pageBack();
    await _settle(tester);
    await tester.scrollUntilVisible(
      find.text('Detailed'),
      -300,
      scrollable: find.byType(Scrollable).first,
    );
    await _settle(tester);
    await tester.tap(find.text('Detailed'));
    await _settle(tester);
    await tester.scrollUntilVisible(
      find.text('Show message times'),
      -200,
      scrollable: find.byType(Scrollable).first,
    );
    await _settle(tester);
    await tester.tap(find.text('Show message times'));
    await _settle(tester);
    expect(state.settings.badgeDetail, BadgeDetail.detailed);
    expect(state.settings.showMessageTimes, isFalse);
    // The core refuses P5 as a floor and keeps a valid default.
    await expectLater(a.setDefaultFloor(floor: 5), throwsA('invalid_floor'));
    await a.setDefaultFloor(floor: 0);
    expect(a.defaultFloor(), 0);
    await a.setDefaultFloor(floor: 2);
    await tester.pageBack();
    await _settle(tester);
    await tester.tap(find.text('Bohdan'));
    await _settle(tester);
    expect(find.text('Received'), findsWidgets);
    expect(find.textContaining('QR · P0 · '), findsWidgets);
    await _shot(tester, '12b-detailed-badges');
    await tester.pageBack();
    await _settle(tester);

    // A new phone restores the backup and is the same identity with the same contacts.
    final file = await a.makeBackup(history: true);
    final c = await openNode(dir: '${tmp.path}/c');
    final restored = await c.restoreWithPhrase(
      data: file,
      words: words.join(' '),
    );
    expect(restored.history, isTrue);
    expect(c.profile().fingerprint, a.profile().fingerprint);
    expect(
      c.contacts().map((x) => x.id).toSet(),
      a.contacts().map((x) => x.id).toSet(),
    );
    // A wrong phrase is refused.
    final d = await openNode(dir: '${tmp.path}/d');
    await expectLater(
      d.restoreWithPhrase(data: file, words: 'not the phrase at all'),
      throwsA(anything),
    );

    // --- Another device: a fresh installation shows its request; A scans it,
    // adds it and shows the setup; the new device reads it and is the same identity, with A's
    // contacts, but not the main device. Then A removes it.
    final e = await openNode(dir: '${tmp.path}/e');
    // Another fresh installation waits too; a setup made for E is not for it.
    final f = await openNode(dir: '${tmp.path}/f');
    final abandonedText = await f.startDeviceJoin();
    final requestText = await e.startDeviceJoin();
    final (rl, rs) = _photograph(qrForText(text: requestText)!);
    final req = await a.scanFrame(
      scanner: newQrScanner(),
      luma: rl,
      width: rs,
      height: rs,
      rowStride: rs,
    );
    final setupQr = (await a.approveDevice(
      request: (req as ScanUpdate_DeviceRequest).field0,
    ))!;
    final setupScanner = newQrScanner();
    ScanUpdate setupUpdate = const ScanUpdate.nothing();
    for (var n = 1; n < 400 && setupUpdate is! ScanUpdate_DeviceSetup; n += 2) {
      final (luma, side) = _photograph(setupQr.frame(n: n)!);
      setupUpdate = await e.scanFrame(
        scanner: setupScanner,
        luma: luma,
        width: side,
        height: side,
        rowStride: side,
      );
    }
    final setup = (setupUpdate as ScanUpdate_DeviceSetup).field0;
    await expectLater(
      f.finishDeviceJoin(setup: setup),
      throwsA('not_for_this_device'),
    );
    await e.finishDeviceJoin(setup: setup);
    // F is approved too, but its setup is never read (the user gave up): it stays waiting.
    final (fl, fs) = _photograph(qrForText(text: abandonedText)!);
    final fReq = await a.scanFrame(
      scanner: newQrScanner(),
      luma: fl,
      width: fs,
      height: fs,
      rowStride: fs,
    );
    await a.approveDevice(request: (fReq as ScanUpdate_DeviceRequest).field0);
    f.cancelDeviceJoin();
    expect(e.isMainDevice(), isFalse);
    expect(e.profile().fingerprint, a.profile().fingerprint);
    expect(
      e.contacts().map((x) => x.id).toSet(),
      a.contacts().map((x) => x.id).toSet(),
    );
    state.changed();
    await _settle(tester);
    await _menu(tester, 'Devices');
    await _settle(tester);
    expect(find.text('This device'), findsOneWidget);
    // E reports in to A at once (over the LAN here); F never does. Without the local network
    // the report cannot travel, so the rest of the device list is checked only with it.
    if (Platform.environment['MEM_IT_SKIP_LAN'] != '1') {
      for (
        var i = 0;
        i < 60 && !a.ownDevices().where((d) => d.joined).skip(1).isNotEmpty;
        i++
      ) {
        await tester.runAsync(
          () => Future<void>.delayed(const Duration(milliseconds: 500)),
        );
      }
      expect(a.ownDevices().where((d) => !d.main && d.joined).length, 1);
      state.changed();
      await _settle(tester);
      expect(find.text('Other device'), findsNWidgets(2));
      expect(find.textContaining('Not joined yet'), findsOneWidget);
      await _shot(tester, '13-devices');
      // Remove the abandoned one: nothing was shared with it.
      final waiting = find.ancestor(
        of: find.textContaining('Not joined yet'),
        matching: find.byType(ListTile),
      );
      await tester.tap(
        find.descendant(of: waiting, matching: find.text('Remove')),
      );
      await _settle(tester);
      expect(find.text('Remove this device?'), findsOneWidget);
      expect(find.textContaining('It never joined'), findsOneWidget);
      await tester.tap(find.text('Remove').last);
      await _settle(tester);
      expect(find.text('Other device'), findsOneWidget);
      expect(find.textContaining('Not joined yet'), findsNothing);
      // Then E, which joined: it is told to forget our MEM.
      await tester.tap(find.text('Remove'));
      await _settle(tester);
      expect(find.textContaining('It never joined'), findsNothing);
      await tester.tap(find.text('Remove').last);
      await _settle(tester);
      expect(find.text('Other device'), findsNothing);
    }
    await tester.pageBack();
    await _settle(tester);

    await tmp.delete(recursive: true);
  });

  testWidgets('confirm in person with a local relying party', (tester) async {
    tester.platformDispatcher.accessibilityFeaturesTestValue =
        const FakeAccessibilityFeatures(disableAnimations: true);
    addTearDown(tester.platformDispatcher.clearAccessibilityFeaturesTestValue);
    final tmp = await Directory.systemTemp.createTemp('mem-it-attest-');
    final a = await openNode(dir: '${tmp.path}/a');
    final b = await openNode(dir: '${tmp.path}/b');
    await a.setLanEnabled(enabled: false);
    await b.setLanEnabled(enabled: false);

    // The relying party: a local server that keeps what it receives.
    final received = <List<int>>[];
    String? contentType;
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    server.listen((request) async {
      contentType = request.headers.contentType?.mimeType;
      received.add(
        await request.fold<List<int>>([], (acc, c) => acc..addAll(c)),
      );
      request.response.statusCode = 201;
      await request.response.close();
    });
    final origin = 'http://127.0.0.1:${server.port}';

    final state = AppState(a);
    await tester.pumpWidget(
      RepaintBoundary(
        key: _boundary,
        child: MemApp(state: state),
      ),
    );
    await _calm(tester);

    // A scans (here: pastes) the request shown by the service.
    await _menu(tester, 'Confirm in person');
    await _settle(tester);
    await tester.enterText(
      find.byKey(const Key('attest-paste')),
      _attestRequest(
        origin: origin,
        role: 'offeror',
        challenge: 1,
        submitUrl: '$origin/mem/attest/1',
      ),
    );
    await tester.tap(find.byKey(const Key('attest-open')));
    await _settle(tester);
    // The disclosure comes first, and says what this gives away.
    expect(find.textContaining('cannot deny it later'), findsOneWidget);
    expect(find.text(origin), findsOneWidget);
    expect(find.textContaining('a4dqobyh'), findsWidgets);
    await _shot(tester, '20-attest-disclosure');
    await tester.tap(find.text('Continue'));
    await _settle(tester);

    // B starts from its own request, then photographs A's code off the screen.
    final flowB = await b.startAttestation(
      text: _attestRequest(
        origin: origin,
        role: 'acceptor',
        challenge: 2,
        submitUrl: '$origin/mem/attest/2',
      ),
      locale: 'en',
    );
    final qrA = tester.widget<QrMatrix>(find.byType(QrMatrix)).qr;
    final (luma, side) = _photograph(qrA);
    final scanned = await b.scanFrame(
      scanner: newQrScanner(),
      luma: luma,
      width: side,
      height: side,
      rowStride: side,
    );
    final emojiB = flowB.acceptPeer(
      text: (scanned as ScanUpdate_Attest).field0,
    );
    await _shot(tester, '21-attest-code');

    // A takes B's code and both show the same emoji.
    await tester.tap(find.text('Scan or paste'));
    await _settle(tester);
    await tester.enterText(
      find.byKey(const Key('attest-paste')),
      flowB.offer().text,
    );
    await tester.tap(find.byKey(const Key('attest-open')));
    await _settle(tester);
    expect(find.text(emojiB), findsOneWidget);
    await _shot(tester, '22-attest-emoji');
    await tester.tap(find.text('They match'));
    await _until(tester, () => received.isNotEmpty);
    await _settle(tester);
    expect(find.textContaining('now has your confirmation'), findsOneWidget);
    expect(contentType, 'application/cbor');
    // B returns its own by the copy/paste path.
    final signedB = flowB.sign();
    expect(signedB.text, startsWith('memattsig1:'));
    expect(() => flowB.sign(), throwsA(anything));

    // The attestation is listed in the privacy log, and nothing secret is.
    expect(a.attestLog().single.origin, origin);
    expect(a.attestLog().single.sent, isTrue);
    await tester.tap(find.text('Done'));
    await _settle(tester);
    await _menu(tester, 'Privacy log');
    await _settle(tester);
    expect(find.text('Confirmed in person'), findsOneWidget);
    await _shot(tester, '23-attest-log');

    await server.close(force: true);
    await tmp.delete(recursive: true);
  });
}
