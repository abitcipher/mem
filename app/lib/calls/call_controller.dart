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
  /// We are calling; the other phone has not confirmed that it rings yet.
  dialing,

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

  /// Ours: the other phone confirmed it rang (or answered). A call that never got there
  /// ends as "not reachable".
  bool reached = false;

  /// Talking, but the media path dropped for now; the call waits for it to come back.
  bool reconnecting = false;

  /// The caller's description, kept until the user accepts.
  String? offerSdp;

  /// Their candidates, kept until our side is ready for them.
  final pending = <String>[];
}

/// One call at a time, carried end to end over MEM: setup messages through the node, media
/// through [MemCallSession]. Navigation to the call screen goes through [navigator].
class CallController extends ChangeNotifier {
  CallController(
    this.node,
    this.settings,
    this.navigator,
    this.openScreen, {
    this.onHistoryChanged,
  });

  final MemNode node;
  final UiSettings settings;
  final GlobalKey<NavigatorState> navigator;

  /// Pushes the call screen (the app's own widget, so this class needs no screen imports).
  final Route<void> Function() openScreen;

  /// Called after a finished call was added to a chat, so open chats show it at once.
  final VoidCallback? onHistoryChanged;

  /// A call nobody answers ends after this long (the user's setting, within its bounds; the
  /// caller cannot make our phone ring longer).
  Duration get ringFor =>
      Duration(seconds: CallTimes.ring.clamp(settings.callRingSeconds));

  /// A call whose other phone does not confirm ringing within this long is not reachable.
  Duration get reachFor =>
      Duration(seconds: CallTimes.reach.clamp(settings.callReachSeconds));

  /// An answered call whose media does not connect within this long has failed.
  static const connectFor = Duration(seconds: 45);

  /// A call in progress whose media stays lost this long ends; until then it waits, and only
  /// the user (or the other side) ends it.
  Duration get dropAfter => Duration(
    seconds: CallTimes.reconnect.clamp(settings.callReconnectSeconds),
  );

  /// Runs while the media is not (or no longer) connected.
  Timer? _media;

  /// The caller retries an ICE restart this often while the media is lost; 4 tries in a
  /// minute stay under the offers a contact accepts per minute.
  static const restartEvery = Duration(seconds: 15);

  /// Restart tries while reconnecting (the caller's side).
  Timer? _restart;

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
      phase: CallPhase.dialing,
    );
    call = c;
    notifyListeners();
    // Dial cue and limit first: the other phone may confirm ringing before the offer call
    // returns, and that confirmation must not be overtaken by them.
    _ring?.cancel();
    _ring = Timer(reachFor, () {
      if (identical(call, c) && !c.reached) {
        unawaited(_finish(CallEndReason.unanswered));
      }
    });
    unawaited(_playCue(MemTone.dial, loop: true));
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
    unawaited(navigator.currentState?.push(openScreen()));
    return null;
  }

  /// The other phone rings: the ringback, and the time it may ring.
  Future<void> _reached(ActiveCall c) async {
    if (c.reached) return;
    c
      ..reached = true
      ..phase = CallPhase.calling;
    notifyListeners();
    _startRinging();
    await _playTone(incoming: false, contactId: c.contactId);
  }

  /// A fixed cue at the outgoing volume.
  Future<void> _playCue(MemTone cue, {required bool loop}) async {
    try {
      await _tones.play(cue, volume: settings.outgoingVolume, loop: loop);
    } on Object {
      // A cue that cannot play must not stop the call.
    }
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
    _connectLimit(c);
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
    call?.phase == CallPhase.calling || call?.phase == CallPhase.dialing
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
          // The other side restarts the media path of the call in progress: answer it.
          if (c.callId == callId &&
              c.contactId == s.contactId &&
              c.talkingSince != null) {
            try {
              final answer = await _session?.answerRestart(
                sdp,
                polite: !c.outgoing,
              );
              if (answer != null) {
                await _send(
                  s.contactId,
                  CallSignalItem.answer(callId: callId, sdp: answer),
                );
              }
            } on Object {
              // The next try may work.
            }
            return;
          }
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
        // Tell the caller it rings here, so it plays its ringback only now.
        unawaited(_send(s.contactId, CallSignalItem.ringing(callId: callId)));
      case CallSignalItem_Ringing(:final callId):
        if (c == null ||
            c.callId != callId ||
            !c.outgoing ||
            c.contactId != s.contactId ||
            c.phase != CallPhase.dialing) {
          return;
        }
        await _reached(c);
      case CallSignalItem_Answer(:final callId, :final sdp):
        if (c == null || c.callId != callId || c.contactId != s.contactId) {
          return;
        }
        // The answer to a restart in a call in progress (either side): just apply it.
        if (c.talkingSince != null) {
          try {
            await _session?.answered(sdp);
          } on Object {
            // A stale answer; the next restart sends a new offer.
          }
          return;
        }
        if (!c.outgoing) return;
        // An answer also proves it got there (an older app sends no "ringing").
        c.reached = true;
        _ring?.cancel();
        await _stopTone();
        c.phase = CallPhase.connecting;
        notifyListeners();
        _connectLimit(c);
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
    // A new call starts on the earpiece unless the user already chose the loudspeaker (it may
    // have been pressed while the call was still ringing).
    unawaited(session.speaker(on: c.speaker));
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
      if (!identical(call, c) || c.phase == CallPhase.ended) return;
      switch (state) {
        case MemCallState.connected:
          _media?.cancel();
          _restart?.cancel();
          // Android picks its default route when audio starts or comes back: apply the
          // user's choice again so the button and the sound agree.
          unawaited(session.speaker(on: c.speaker));
          c
            ..phase = CallPhase.talking
            ..reconnecting = false;
          c.talkingSince ??= DateTime.now();
          notifyListeners();
        case MemCallState.interrupted || MemCallState.failed
            when c.talkingSince != null:
          // In a call: never end on a hiccup. Wait for the media to come back; only a long
          // loss ends it (the user can end it any time).
          if (!c.reconnecting) {
            c.reconnecting = true;
            notifyListeners();
            _media?.cancel();
            _media = Timer(dropAfter, () {
              if (identical(call, c) && c.reconnecting) {
                unawaited(_finish(CallEndReason.failed));
              }
            });
            // The caller asks for a fresh path; the callee answers it (a lost path does not
            // come back by itself once WebRTC has given it up).
            // Both sides try, each while it is still cut off; the callee a little later, so
            // that the two rarely offer at once.
            _restart?.cancel();
            final first = Duration(seconds: c.outgoing ? 3 : 9);
            _restart = Timer(first, () {
              unawaited(_restartIce(c));
              _restart = Timer.periodic(restartEvery, (_) => _restartIce(c));
            });
          }
        case MemCallState.failed:
          // Never connected: it cannot.
          unawaited(_finish(CallEndReason.failed));
        case MemCallState.interrupted ||
            MemCallState.connecting ||
            MemCallState.closed:
          break;
      }
    };
    return session;
  }

  Future<void> _restartIce(ActiveCall c) async {
    final session = _session;
    if (!identical(call, c) || !c.reconnecting || session == null) {
      _restart?.cancel();
      return;
    }
    try {
      final sdp = await session.restartOffer();
      await node.sendCallSignal(
        contactId: c.contactId,
        signal: CallSignalItem.offer(callId: c.callId, sdp: sdp),
      );
    } on Object {
      // No channel right now: the next try may find one.
    }
  }

  /// An answered call must connect its media within [connectFor].
  void _connectLimit(ActiveCall c) {
    _media?.cancel();
    _media = Timer(connectFor, () {
      if (identical(call, c) && c.talkingSince == null) {
        unawaited(_finish(CallEndReason.failed));
      }
    });
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
    _media?.cancel();
    _restart?.cancel();
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
    // The loudspeaker is a phone-wide setting: never leave it on for the next call.
    if (c.speaker) {
      try {
        await _session?.speaker(on: false);
      } on Object {
        // Nothing to switch back.
      }
    }
    // A cue when the call did not happen or broke off, not when someone just hung up.
    final broke =
        (c.outgoing && !c.reached) ||
        reason == CallEndReason.failed ||
        (!tell && reason != CallEndReason.hungUp);
    if (broke) unawaited(_playCue(MemTone.unavailable, loop: false));
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
        : c.outgoing && !c.reached
        ? CallOutcomeItem.unreachable
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
                // Direct between the devices, no server: the same grade as the local network.
                MemCallPath.localNetwork || MemCallPath.direct => Channel.lan,
                MemCallPath.servers => Channel.hub,
                null => null,
              },
      );
      onHistoryChanged?.call();
    } on Object {
      // History is a convenience; a failure must not disturb the call.
    }
  }

  @override
  void dispose() {
    _ring?.cancel();
    _media?.cancel();
    _restart?.cancel();
    unawaited(_tones.dispose());
    unawaited(_session?.close());
    super.dispose();
  }
}
