import 'dart:async';
import 'dart:io';

import 'package:flutter/widgets.dart';
import 'package:mem_push/mem_push.dart';

import 'rust/api.dart';
import 'ui_settings.dart';

/// Push wake-ups: the UnifiedPush distributor gives an endpoint, the node registers
/// it with the hub, and a wake-up means "fetch from the hub now". If something arrived while
/// the app is not on screen, a notification without names or text says so.
class PushBridge {
  PushBridge(this.node, this.settings, {required this.texts});

  final MemNode node;
  final UiSettings settings;

  /// Localized notification title and text, set once the UI knows its language.
  ({String title, String text}) Function() texts;

  static bool get available => Platform.isAndroid;

  /// How long a wake-up may take before the app gives the system back its time.
  static const _window = Duration(seconds: 20);

  bool _waking = false;

  Future<void> attach() async {
    if (!available) return;
    MemPush.listen(onEndpoint: _endpoint, onWake: _wake);
    // The distributor may have changed the endpoint while we were not running.
    if (settings.push) {
      final e = await MemPush.endpoint();
      if (e != null && e != node.pushEndpoint()) await _endpoint(e);
    }
    if (await MemPush.takePendingWake()) await _wake(headless: true);
  }

  /// Turns wake-ups on (registering with [distributor], or the saved one) or off.
  Future<void> enable({required bool on, String? distributor}) async {
    settings
      ..push = on
      ..write();
    if (on) {
      await MemPush.register(distributor: distributor);
    } else {
      await MemPush.unregister();
      await node.setPushEndpoint(endpoint: null);
    }
  }

  Future<void> _endpoint(String? url) async {
    // Only while the user wants wake-ups; a late callback after turning off is ignored.
    final wanted = settings.push ? url : null;
    try {
      await node.setPushEndpoint(endpoint: wanted);
    } on Object {
      // An endpoint the hub may not call (not https): wake-ups stay off.
    }
  }

  /// A headless wake-up starts the app, whose node fetches from the hub during start-up, before
  /// this bridge exists. The app was not running before, so anything that arrived this recently
  /// came with this wake-up.
  static const _headlessLookback = Duration(minutes: 2);

  Future<void> _wake({bool headless = false}) async {
    if (_waking) return MemPush.wakeDone();
    _waking = true;
    final now = DateTime.now();
    final since = (headless ? now.subtract(_headlessLookback) : now)
        .millisecondsSinceEpoch;
    try {
      // The hub connects by itself when it is on; give the fetch time to complete.
      final end = DateTime.now().add(_window);
      var found = false;
      while (DateTime.now().isBefore(end)) {
        await Future<void>.delayed(const Duration(milliseconds: 500));
        if (_arrivedSince(since)) {
          found = true;
          // Let a burst finish arriving.
          await Future<void>.delayed(const Duration(seconds: 2));
          break;
        }
      }
      final onScreen =
          WidgetsBinding.instance.lifecycleState == AppLifecycleState.resumed;
      if (found && !onScreen) {
        final t = texts();
        await MemPush.notify(title: t.title, text: t.text);
      }
    } finally {
      _waking = false;
      await MemPush.wakeDone();
    }
  }

  /// Whether any message (1:1 or group) arrived at or after [since], Unix ms.
  bool _arrivedSince(int since) {
    bool fresh(MessageItem m) =>
        !m.outgoing &&
        m.channels.isNotEmpty &&
        m.channels.last.atUnixMs.toInt() >= since;
    for (final c in node.contacts()) {
      if (node.messages(contactId: c.id).any(fresh)) return true;
    }
    for (final g in node.groups()) {
      if (node.groupMessages(groupId: g.id).any(fresh)) return true;
    }
    return false;
  }
}
