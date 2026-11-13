import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:mem/l10n/gen/app_localizations.dart';
import 'package:mem/src/rust/api.dart';
import 'package:mem/widgets/attachments.dart';

MessageItem _with(List<AttachmentItem> attachments) => MessageItem(
  id: 'm1',
  outgoing: true,
  isRequest: false,
  text: '',
  createdUnixMs: 0,
  status: MessageStatus.sent,
  channels: const [],
  senderId: 'peer',
  attachments: attachments,
  edited: false,
  editable: false,
);

AttachmentItem _file(AttachmentStatus state) => AttachmentItem(
  name: 'report.pdf',
  mediaType: 'application/pdf',
  size: BigInt.from(150 * 1024),
  state: state,
);

void main() {
  Future<void> show(WidgetTester tester, MessageItem item) => tester.pumpWidget(
    MaterialApp(
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      home: Scaffold(
        body: AttachmentList(item: item, background: Colors.white),
      ),
    ),
  );

  testWidgets('a file going out shows its name and progress', (tester) async {
    await show(
      tester,
      _with([_file(const AttachmentStatus.sending(sent: 2, total: 5))]),
    );
    expect(find.text('report.pdf'), findsOneWidget);
    expect(find.text('Sending 2 of 5'), findsOneWidget);
    expect(find.byType(LinearProgressIndicator), findsOneWidget);
  });

  testWidgets('a file here shows its size and no progress', (tester) async {
    await show(tester, _with([_file(const AttachmentStatus.ready())]));
    expect(find.text('150 KB'), findsOneWidget);
    expect(find.byType(LinearProgressIndicator), findsNothing);
  });

  test('media types and sizes', () {
    expect(mediaTypeFor('IMG.JPG'), 'image/jpeg');
    expect(mediaTypeFor('noext'), 'application/octet-stream');
    expect(formatSize(900), '900 B');
    expect(formatSize(2048), '2 KB');
    expect(formatSize(3 * 1024 * 1024 + 100000), '3.1 MB');
  });
}
