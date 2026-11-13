import 'dart:io';

import 'package:cupertino_ui/cupertino_ui.dart'
    show GlobalCupertinoLocalizations;
import 'package:flutter_localizations/flutter_localizations.dart'
    show GlobalWidgetsLocalizations;
import 'package:material_ui/material_ui.dart';
import 'package:flutter_chat_core/flutter_chat_core.dart' as fc;
import 'package:flutter_test/flutter_test.dart';
import 'package:mem/l10n/gen/app_localizations.dart';
import 'package:mem/src/rust/api.dart';
import 'package:mem/src/ui_settings.dart';
import 'package:mem/widgets/grade_badge.dart';
import 'package:mem/widgets/grade_symbol.dart';
import 'package:mem/widgets/message_bubble.dart';

/// The app's own delegates (the generated list still names the pre-split Material ones).
const _delegates = <LocalizationsDelegate<dynamic>>[
  AppLocalizations.delegate,
  GlobalMaterialLocalizations.delegate,
  GlobalWidgetsLocalizations.delegate,
  GlobalCupertinoLocalizations.delegate,
];

const _t0 = 1800000000000;
var _now = _t0;
var _ids = 0;

MessageItem _message({
  required List<String> grades,
  int arrivedAgo = 0,
  bool outgoing = false,
}) => MessageItem(
  id: 'm${_ids++}',
  outgoing: outgoing,
  isRequest: false,
  text: 'hello',
  attachments: const [],
  createdUnixMs: _now - arrivedAgo,
  status: outgoing ? MessageStatus.delivered : MessageStatus.received,
  channels: [
    for (final g in grades)
      ChannelUseItem(
        channel: g == 'P5' ? Channel.plain : Channel.lan,
        grade: g,
        atUnixMs: _now - arrivedAgo,
      ),
  ],
  senderId: 'peer',
);

Future<void> _show(
  WidgetTester tester,
  MessageItem item, {
  required int mode,
  int? floor,
}) async {
  final settings = UiSettings.read(null)..badgeMode = mode;
  await tester.pumpWidget(
    MaterialApp(
      localizationsDelegates: _delegates,
      supportedLocales: AppLocalizations.supportedLocales,
      home: Scaffold(
        body: Center(
          child: SizedBox(
            width: 300,
            child: Indication(
              key: ValueKey(item.id),
              item: item,
              background: Colors.white,
              fontSize: 11,
              settings: settings,
              floor: floor,
            ),
          ),
        ),
      ),
    ),
  );
  await tester.pump();
}

/// Moves the indication's clock and the test's timers forward together.
Future<void> _wait(WidgetTester tester, Duration d) async {
  _now += d.inMilliseconds;
  await tester.pump(d);
  await tester.pump(const Duration(milliseconds: 300));
  // Let the switch between full and minimal finish.
  await tester.pump(const Duration(milliseconds: 300));
}

void main() {
  setUp(() {
    _now = _t0;
    Indication.now = () => _now;
  });

  bubbleTests();

  group('settings', () {
    test('slider positions map to delays', () {
      expect(UiSettings.badgeDelay(1), isNull);
      expect(UiSettings.badgeDelay(2), isNull);
      expect(UiSettings.badgeDelay(3), const Duration(seconds: 3));
      expect(UiSettings.badgeDelay(5), const Duration(seconds: 15));
      expect(UiSettings.badgeDelay(13), const Duration(hours: 1));
      expect(UiSettings.badgeDelay(14), isNull);
    });

    test('defaults, bounds and round trip', () {
      final dir = Directory.systemTemp.createTempSync('mem-ui-');
      addTearDown(() => dir.deleteSync(recursive: true));
      final file = File('${dir.path}/ui')
        ..writeAsStringSync(
          'badge_mode=99\nsymbol_style=oneShape\nsymbol_shape=slash',
        );
      final s = UiSettings.read(file);
      expect(s.badgeMode, UiSettings.maxBadgeMode);
      expect(s.symbolStyle, SymbolStyle.oneShape);
      // P4's shape is not one the user may pick for P0 to P3.
      expect(s.symbolShape, SymbolShape.fisheye);
      s
        ..badgeMode = 2
        ..write();
      expect(UiSettings.read(file).badgeMode, 2);
      expect(UiSettings.read(null).badgeMode, 5);
    });

    test('P4 and P5 keep their own shapes in every style', () {
      for (final style in SymbolStyle.values) {
        for (final chosen in oneShapeChoices) {
          expect(shapeFor('P4', style, chosen), SymbolShape.slash);
          expect(shapeFor('P5', style, chosen), SymbolShape.cross);
          expect(shapeFor('P2', style, chosen), isNot(SymbolShape.slash));
          expect(shapeFor('P2', style, chosen), isNot(SymbolShape.cross));
        }
      }
      expect(
        {
          for (var g = 0; g < 6; g++)
            shapeFor('P$g', SymbolStyle.byGrade, SymbolShape.fisheye),
        }.length,
        6,
      );
    });
  });

  group('indication', () {
    testWidgets('a new message shows full, then collapses after the delay', (
      tester,
    ) async {
      await _show(tester, _message(grades: ['P1']), mode: 5);
      expect(find.byType(GradeBadge), findsOneWidget);
      await _wait(tester, const Duration(seconds: 14));
      expect(find.byType(GradeBadge), findsOneWidget);
      await _wait(tester, const Duration(seconds: 2));
      expect(find.byType(GradeBadge), findsNothing);
      expect(find.byType(GradeSymbol), findsOneWidget);
    });

    testWidgets('a message older than the delay opens collapsed', (
      tester,
    ) async {
      final old = _message(grades: ['P1'], arrivedAgo: 60000);
      await _show(tester, old, mode: 5);
      expect(find.byType(GradeBadge), findsNothing);
      expect(find.byType(GradeSymbol), findsOneWidget);
    });

    testWidgets('tapping the badge area shows them in full for a while', (
      tester,
    ) async {
      final old = _message(grades: ['P1', 'P2'], arrivedAgo: 60000);
      await _show(tester, old, mode: 4);
      expect(find.byType(GradeSymbol), findsNWidgets(2));
      await tester.tap(find.byType(Indication));
      await tester.pump(const Duration(milliseconds: 300));
      expect(find.byType(GradeBadge), findsNWidgets(2));
      await _wait(tester, const Duration(seconds: 8));
      expect(find.byType(GradeBadge), findsNothing);
    });

    testWidgets('symbol only shows every grade; tap shows full for 7 s', (
      tester,
    ) async {
      await _show(tester, _message(grades: ['P0']), mode: 2);
      expect(find.byType(GradeSymbol), findsOneWidget);
      await tester.tap(find.byType(Indication));
      await tester.pump(const Duration(milliseconds: 300));
      expect(find.byType(GradeBadge), findsOneWidget);
      await _wait(tester, Indication.tapShows);
      expect(find.byType(GradeBadge), findsNothing);
    });

    testWidgets('position 1 shows only P4, P5 and grades below the floor', (
      tester,
    ) async {
      await _show(tester, _message(grades: ['P1']), mode: 1, floor: 3);
      expect(find.byType(GradeSymbol), findsNothing);
      expect(find.byType(GradeBadge), findsNothing);

      await _show(tester, _message(grades: ['P5']), mode: 1, floor: 3);
      expect(find.byType(GradeSymbol), findsOneWidget);

      await _show(tester, _message(grades: ['P2']), mode: 1, floor: 1);
      expect(find.byType(GradeSymbol), findsOneWidget);

      // Group chats have no floor: only P4 and P5.
      await _show(tester, _message(grades: ['P3', 'P4']), mode: 1);
      expect(find.byType(GradeSymbol), findsOneWidget);

      // The full badges stay one tap away.
      await tester.tap(find.byType(Indication));
      await tester.pump(const Duration(milliseconds: 300));
      expect(find.byType(GradeBadge), findsNWidgets(2));
    });

    testWidgets('a channel used several times shows once', (tester) async {
      await _show(tester, _message(grades: ['P1', 'P1', 'P1', 'P5']), mode: 14);
      expect(find.byType(GradeBadge), findsNWidgets(2));
    });

    testWidgets('always full ignores taps', (tester) async {
      final old = _message(grades: ['P1'], arrivedAgo: 600000);
      await _show(tester, old, mode: 14);
      expect(find.byType(GradeBadge), findsOneWidget);
      await tester.tap(find.byType(Indication));
      await _wait(tester, const Duration(hours: 2));
      expect(find.byType(GradeBadge), findsOneWidget);
    });

    testWidgets('screen readers get the full text in the minimal form', (
      tester,
    ) async {
      final handle = tester.ensureSemantics();
      final old = _message(grades: ['P5'], arrivedAgo: 60000);
      await _show(tester, old, mode: 5);
      expect(find.byType(GradeBadge), findsNothing);
      expect(find.bySemanticsLabel(RegExp('P5')), findsOneWidget);
      handle.dispose();
    });
  });
}

Future<MessageBubble> _bubble(
  WidgetTester tester,
  MessageItem item, {
  required double width,
  int mode = 14,
}) async {
  final settings = UiSettings.read(null)..badgeMode = mode;
  final bubble = MessageBubble(
    message: fc.Message.text(
      id: item.id,
      authorId: 'me',
      createdAt: DateTime.fromMillisecondsSinceEpoch(_now),
      text: 'Hi',
    ) as fc.TextMessage,
    isSentByMe: true,
    item: item,
    settings: settings,
  );
  await tester.pumpWidget(
    MaterialApp(
      localizationsDelegates: _delegates,
      supportedLocales: AppLocalizations.supportedLocales,
      home: Scaffold(
        body: Align(
          alignment: Alignment.topLeft,
          child: ConstrainedBox(
            constraints: BoxConstraints(maxWidth: width),
            child: bubble,
          ),
        ),
      ),
    ),
  );
  await tester.pump();
  return bubble;
}

void bubbleTests() {
  group('bubble layout', () {
    setUp(() {
      _now = _t0;
      Indication.now = () => _now;
    });

    testWidgets('badges stay on one line and widen a short bubble', (
      tester,
    ) async {
      await _bubble(tester, _message(grades: ['P0', 'P1', 'P2']), width: 700);
      final tops = {
        for (final e in find.byType(GradeBadge).evaluate())
          tester.getTopLeft(find.byWidget(e.widget)).dy,
      };
      expect(tops.length, 1, reason: 'all badges on one line');
      // The text is short; the bubble is as wide as the badge line.
      final bubble = tester.getRect(find.byType(MessageBubble));
      final lastBadge = find.byType(GradeBadge).last;
      expect(bubble.right, greaterThan(tester.getRect(lastBadge).right));
    });

    testWidgets('badges are anchored to the bottom left of the bubble', (
      tester,
    ) async {
      await _bubble(tester, _message(grades: ['P1']), width: 700);
      final bubble = tester.getRect(find.byType(MessageBubble));
      final badge = tester.getRect(find.byType(GradeBadge));
      expect(badge.left - bubble.left, lessThan(24));
      final old = _message(grades: ['P1', 'P4'], arrivedAgo: 60000);
      await _bubble(tester, old, width: 700, mode: 5);
      final b2 = tester.getRect(find.byType(MessageBubble));
      final first = tester.getRect(find.byType(GradeSymbol).first);
      expect(first.left - b2.left, lessThan(24));
      expect(
        tester.getRect(find.byType(GradeSymbol).last).left,
        greaterThan(first.left),
      );
    });

    testWidgets('badges stack only when the screen is too narrow', (
      tester,
    ) async {
      await _bubble(tester, _message(grades: ['P0', 'P1', 'P2']), width: 200);
      final tops = {
        for (final e in find.byType(GradeBadge).evaluate())
          tester.getTopLeft(find.byWidget(e.widget)).dy,
      };
      expect(tops.length, greaterThan(1));
      expect(tester.takeException(), isNull);
    });
  });
}
