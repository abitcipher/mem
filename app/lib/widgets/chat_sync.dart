import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter_chat_core/flutter_chat_core.dart' as fc;

/// Makes [controller] show [next].
///
/// A bubble redraws only when its own message is updated; replacing the whole list adds and
/// removes bubbles but leaves a changed one (a new status) as it was drawn. So changed messages
/// are updated one by one, and the list is replaced only when messages were added or removed.
void showMessages(fc.InMemoryChatController controller, List<fc.Message> next) {
  final shown = {for (final m in controller.messages) m.id: m};
  for (final m in next) {
    final old = shown[m.id];
    if (old != null && old != m) unawaited(controller.updateMessage(old, m));
  }
  final sameMessages = listEquals(
    [for (final m in controller.messages) m.id],
    [for (final m in next) m.id],
  );
  if (!sameMessages) unawaited(controller.setMessages(next));
}
