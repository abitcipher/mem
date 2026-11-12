import 'package:material_ui/material_ui.dart';
import 'package:flutter_chat_core/flutter_chat_core.dart' as fc;
import 'package:flutter_chat_ui/flutter_chat_ui.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets('flutter_chat_ui composer builds inside the app shell', (
    tester,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        localizationsDelegates: const [GlobalMaterialLocalizations.delegate],
        home: Scaffold(
          body: Material(
            type: MaterialType.transparency,
            child: Chat(
              currentUserId: 'me',
              chatController: fc.InMemoryChatController(),
              resolveUser: (id) async => fc.User(id: id),
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
    expect(find.byType(TextField), findsOneWidget);
  });
}
