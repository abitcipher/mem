import 'package:material_ui/material_ui.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mem/app_scope.dart';

void main() {
  // Regression: "Already in your contacts" (with a "Compare symbols" action) stayed
  // over the chat's text field, because Flutter keeps snack bars with an action by default.
  testWidgets('a snack bar with an action goes away by itself', (tester) async {
    late BuildContext ctx;
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: Builder(
            builder: (context) {
              ctx = context;
              return const SizedBox.expand();
            },
          ),
        ),
      ),
    );
    showSnack(
      ctx,
      'Already in your contacts',
      action: SnackBarAction(label: 'Compare symbols', onPressed: () {}),
    );
    await tester.pumpAndSettle();
    expect(find.text('Already in your contacts'), findsOneWidget);
    await tester.pump(const Duration(seconds: 10));
    await tester.pumpAndSettle();
    expect(find.text('Already in your contacts'), findsNothing);
  });
}
