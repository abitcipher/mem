import 'dart:async';
import 'dart:math';

import 'package:flutter/foundation.dart';

import 'package:material_ui/material_ui.dart';
import 'package:mem_calls/mem_calls.dart';
import 'package:mem_radio/mem_radio.dart';

import '../src/rust/api.dart';
import '../src/ui_settings.dart';

/// Where a call stands, as the user sees it.
enum CallPhase {
  /// We are calling; the other phone rings.
  calling,

  /// Their call rings here.
  ringing,

  /// Accepted; the media is connecting.
  connecting,

  /// Talking.
  talking,

  /// Over (shown briefly, then the screen closes).
  ended,
}

/// The call in progress, if any.
class ActiveCall {
  ActiveCall({
    required this.contactId,
    required this.callId,
    required this.outgoing,
    required this.phase,
  });

  final String contactId;
  final String callId;
  final bool outgoing;
  CallPhase phase;

  /// When it started ringing.
  final DateTime started = DateTime.now();

  /// How the media goes, once connected.
  MemCallPath? path;
  bool muted = false;
  bool speaker = false;
  DateTime? talkingSince;

  /// Why it ended, when it did.
  CallEndReason? endReason;

  /// The caller's description, kept until the user accepts.
  String? offerSdp;

  /// Their candidates, kept until our side is ready for them.
  final pending = <String>[];
}

/// One call at a time, carried end to end over MEM: setup messages through the node, media
/// through [MemCallSession]. Navigation to the call screen goes through [navigator].
class CallController extends ChangeNotifier {
  CallController(this.node, this.settings, this.navigator, this.openScreen);

  final MemNode node;
  final UiSettings settings;
  final GlobalKey<NavigatorState> navigator;

  /// Pushes the call screen (the app's own widget, so this class needs no screen imports).
  final Route<void> Function() openScreen;

  /// A call nobody answers ends after this long.
  static const ringFor = Duration(seconds: 45);

  ActiveCall? call;
  MemCallSession? _session;
  final _tones = MemTonePlayer();
  Timer? _ring;
  final _random = Random.secure();

  /// Whether calls can be made here at all (Android in this version; the setting decides
  /// whether they are on).
  static bool get supported => defaultTargetPlatform == TargetPlatform.android;

  bool get enabled => supported && settings.voiceCalls;

  String _newCallId() =>
      [for (var i = 0; i < 16; i++) _random.nextInt(256)]
          .map((b) => b.toRadixString(16).padLeft(2, '0'))
          .join();

  /// Calls [contactId]. Returns a reason key when it cannot start (`unreachable`, no
  /// microphone, already in a call), or `null` once it is ringing out.
  Future<String?> start(String contactId) async {
    if (!enabled) return 'unsupported';
    if (call != null) return 'busy';
    if (!await MemRadio.soundRequestPermission()) return 'no_microphone';
    final c = ActiveCall(
      contactId: contactId,
      callId: _newCallId(),
      outgoing: true,
      phase: CallPhase.calling,
    );
    call = c;
    notifyListeners();
    try {
      final session = await _open(c);
      final sdp = await session.offer();
      await node.sendCallSignal(
        contactId: contactId,
        signal: CallSignalItem.offer(callId: c.callId, sdp: sdp),
      );
    } on Object catch (e) {
      await _finish(CallEndReason.failed, tell: false);
      return e.toString();
    }
    _startRinging();
    unawaited(_playTone(incoming: false, contactId: contactId));
    unawaited(navigator.currentState?.push(openScreen()));
    return null;
  }

  /// The tone for an incoming call from [contactId] (its own choice, else the default), or
  /// the ringback for our call. "off" or zero volume plays nothing.
  Future<void> _playTone({
    required bool incoming,
    required String contactId,
  }) async {
    final String id;
    final double volume;
    if (incoming) {
      final own = node
          .contacts()
          .where((c) => c.id == contactId)
          .firstOrNull
          ?.ringtone;
      id = own ?? settings.incomingTone;
      volume = settings.incomingVolume;
    } else {
      id = settings.outgoingTone;
      volume = settings.outgoingVolume;
    }
    final tone = MemTone.byId(id);
    if (tone == null) return;
    try {
      await _tones.play(tone, volume: volume);
    } on Object {
      // A tone that cannot play must not stop the call.
    }
  }

  Future<void> _stopTone() async {
    try {
      await _tones.stop();
    } on Object {
      // Nothing playing.
    }
  }

  /// Accepts the ringing call.
  Future<void> accept() async {
    final c = call;
    if (c == null || c.phase != CallPhase.ringing) return;
    if (!await MemRadio.soundRequestPermission()) {
      await decline();
      return;
    }
    _ring?.cancel();
    await _stopTone();
    c.phase = CallPhase.connecting;
    notifyListeners();
    try {
      final session = await _open(c);
      final sdp = await session.answer(c.offerSdp ?? '');
      await node.sendCallSignal(
        contactId: c.contactId,
        signal: CallSignalItem.answer(callId: c.callId, sdp: sdp),
      );
      await session.addCandidates(List.of(c.pending));
      c.pending.clear();
    } on Object {
      await _finish(CallEndReason.failed);
    }
  }

  /// Declines the ringing call.
  Future<void> decline() => _finish(CallEndReason.declined);

  /// Ends the call (or stops calling).
  Future<void> hangUp() => _finish(
    call?.phase == CallPhase.calling
        ? CallEndReason.unanswered
        : CallEndReason.hungUp,
  );

  void toggleMute() {
    final c = call;
    if (c == null) return;
    c.muted = !c.muted;
    _session?.mute(muted: c.muted);
    notifyListeners();
  }

  Future<void> toggleSpeaker() async {
    final c = call;
    if (c == null) return;
    c.speaker = !c.speaker;
    await _session?.speaker(on: c.speaker);
    notifyListeners();
  }

  /// New setup messages arrived ([NodeEventItem_Call]).
  Future<void> onSignals() async {
    for (final s in node.takeCallSignals()) {
      await _handle(s);
    }
  }

  Future<void> _handle(IncomingCallItem s) async {
    final c = call;
    switch (s.signal) {
      case CallSignalItem_Offer(:final callId, :final sdp):
        if (!enabled) return;
        if (c != null) {
          // One call at a time.
          if (c.callId != callId) {
            await _send(
              s.contactId,
              CallSignalItem.end(callId: callId, reason: CallEndReason.busy),
            );
          }
          return;
        }
        call = ActiveCall(
          contactId: s.contactId,
          callId: callId,
          outgoing: false,
          phase: CallPhase.ringing,
        )..offerSdp = sdp;
        _startRinging();
        unawaited(_playTone(incoming: true, contactId: s.contactId));
        notifyListeners();
        unawaited(navigator.currentState?.push(openScreen()));
      case CallSignalItem_Answer(:final callId, :final sdp):
        if (c == null || c.callId != callId || !c.outgoing) return;
        _ring?.cancel();
        await _stopTone();
        c.phase = CallPhase.connecting;
        notifyListeners();
        await _session?.answered(sdp);
      case CallSignalItem_Candidates(:final callId, :final candidates):
        if (c == null || c.callId != callId || c.contactId != s.contactId) {
          return;
        }
        final session = _session;
        if (session == null) {
          c.pending.addAll(candidates);
        } else {
          await session.addCandidates(candidates);
        }
      case CallSignalItem_End(:final callId, :final reason):
        if (c == null || c.callId != callId || c.contactId != s.contactId) {
          return;
        }
        await _finish(reason, tell: false);
    }
  }

  Future<MemCallSession> _open(ActiveCall c) async {
    final session = await MemCallSession.open();
    _session = session;
    session.onCandidates = (lines) => unawaited(
      _send(
        c.contactId,
        CallSignalItem.candidates(callId: c.callId, candidates: lines),
      ),
    );
    session.onPath = (path) {
      c.path = path;
      notifyListeners();
    };
    session.onState = (state) {
      switch (state) {
        case MemCallState.connected:
          c.phase = CallPhase.talking;
          c.talkingSince ??= DateTime.now();
          notifyListeners();
        case MemCallState.failed:
          unawaited(_finish(CallEndReason.failed));
        case MemCallState.connecting || MemCallState.closed:
          break;
      }
    };
    return session;
  }

  void _startRinging() {
    _ring?.cancel();
    _ring = Timer(ringFor, () => unawaited(_finish(CallEndReason.unanswered)));
  }

  Future<void> _send(String contactId, CallSignalItem signal) async {
    try {
      await node.sendCallSignal(contactId: contactId, signal: signal);
    } on Object {
      // The other side's own timers end the call if this does not get through.
    }
  }

  /// Ends the call for [reason]; tells the other side unless it told us.
  Future<void> _finish(CallEndReason reason, {bool tell = true}) async {
    final c = call;
    if (c == null || c.phase == CallPhase.ended) return;
    _ring?.cancel();
    await _stopTone();
    if (tell) {
      await _send(
        c.contactId,
        CallSignalItem.end(callId: c.callId, reason: reason),
      );
    }
    c
      ..phase = CallPhase.ended
      ..endReason = reason;
    final session = _session;
    _session = null;
    await session?.close();
    await _record(c, reason);
    notifyListeners();
    // The screen shows the ending for a moment, then the call is gone.
    await Future<void>.delayed(const Duration(seconds: 2));
    if (identical(call, c)) {
      call = null;
      notifyListeners();
    }
  }

  /// Keeps the call as a line in the chat, when the user keeps call history.
  Future<void> _record(ActiveCall c, CallEndReason reason) async {
    if (!settings.callHistory) return;
    final talked = c.talkingSince;
    final outcome = talked != null
        ? CallOutcomeItem.answered
        : switch (reason) {
            CallEndReason.declined => CallOutcomeItem.declined,
            CallEndReason.busy => CallOutcomeItem.busy,
            CallEndReason.failed => CallOutcomeItem.failed,
            // Rang out, or stopped before an answer.
            CallEndReason.unanswered || CallEndReason.hungUp =>
              c.outgoing ? CallOutcomeItem.unanswered : CallOutcomeItem.missed,
          };
    try {
      await node.recordCall(
        contactId: c.contactId,
        outgoing: c.outgoing,
        startedUnixMs: c.started.millisecondsSinceEpoch,
        durationS: talked == null
            ? 0
            : DateTime.now().difference(talked).inSeconds,
        outcome: outcome,
        via: talked == null
            ? null
            : switch (c.path) {
                MemCallPath.wifiDirect => Channel.wifiDirect,
                MemCallPath.localNetwork => Channel.lan,
                MemCallPath.servers => Channel.hub,
                null => null,
              },
      );
    } on Object {
      // History is a convenience; a failure must not disturb the call.
    }
  }

  @override
  void dispose() {
    _ring?.cancel();
    unawaited(_tones.dispose());
    unawaited(_session?.close());
    super.dispose();
  }
}
