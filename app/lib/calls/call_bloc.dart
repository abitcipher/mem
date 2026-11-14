import 'dart:async';
import 'dart:math';

import 'package:bloc/bloc.dart';
import 'package:bloc_concurrency/bloc_concurrency.dart';
import 'package:mem_calls/mem_calls.dart';

import '../src/rust/api.dart';

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

/// The call in progress, as the screens see it. Immutable: every change is a new value.
class ActiveCall {
  const ActiveCall({
    required this.contactId,
    required this.callId,
    required this.outgoing,
    required this.phase,
    required this.started,
    this.path,
    this.muted = false,
    this.speaker = false,
    this.talkingSince,
    this.endReason,
    this.reached = false,
    this.reconnecting = false,
  });

  final String contactId;
  final String callId;
  final bool outgoing;
  final CallPhase phase;

  /// When it started ringing.
  final DateTime started;

  /// How the media goes, once connected.
  final MemCallPath? path;
  final bool muted;
  final bool speaker;
  final DateTime? talkingSince;

  /// Why it ended, when it did.
  final CallEndReason? endReason;

  /// Ours: the other phone confirmed it rang (or answered). A call that never got there
  /// ends as "not reachable".
  final bool reached;

  /// Talking, but the media path dropped for now; the call waits for it to come back.
  final bool reconnecting;

  ActiveCall copyWith({
    CallPhase? phase,
    MemCallPath? path,
    bool? muted,
    bool? speaker,
    DateTime? talkingSince,
    CallEndReason? endReason,
    bool? reached,
    bool? reconnecting,
  }) => ActiveCall(
    contactId: contactId,
    callId: callId,
    outgoing: outgoing,
    phase: phase ?? this.phase,
    started: started,
    path: path ?? this.path,
    muted: muted ?? this.muted,
    speaker: speaker ?? this.speaker,
    talkingSince: talkingSince ?? this.talkingSince,
    endReason: endReason ?? this.endReason,
    reached: reached ?? this.reached,
    reconnecting: reconnecting ?? this.reconnecting,
  );
}

/// The bloc's state: the call, if there is one.
class CallState {
  const CallState([this.call]);

  final ActiveCall? call;
}

/// The sounds of a call. Which file and volume each one is stays the app's choice.
enum CallSound {
  /// Ours, until the other phone rings (looping).
  dial,

  /// Ours, while the other phone rings (looping).
  ringback,

  /// Theirs, ringing here (looping).
  ringtone,

  /// The call did not happen or broke off (once).
  unavailable,
}

/// The time limits of a call, read when each one starts.
class CallLimits {
  const CallLimits({
    required this.ringFor,
    required this.reachFor,
    required this.dropAfter,
    this.connectFor = const Duration(seconds: 45),
    this.restartEvery = const Duration(seconds: 15),
  });

  /// A call nobody answers ends after this long.
  final Duration ringFor;

  /// A call whose other phone does not confirm ringing within this long is not reachable.
  final Duration reachFor;

  /// A call in progress whose media stays lost this long ends.
  final Duration dropAfter;

  /// An answered call whose media does not connect within this long has failed.
  final Duration connectFor;

  /// While the media is lost, a fresh path is asked for this often (4 tries a minute stay
  /// under the offers a contact accepts per minute).
  final Duration restartEvery;
}

/// One finished call, for the chat's history.
class CallRecord {
  const CallRecord({
    required this.contactId,
    required this.outgoing,
    required this.started,
    required this.durationS,
    required this.outcome,
    required this.via,
  });

  final String contactId;
  final bool outgoing;
  final DateTime started;
  final int durationS;
  final CallOutcomeItem outcome;
  final Channel? via;
}

/// One call's media, as the bloc uses it ([MemCallSession] in the app, a fake in tests).
abstract interface class CallMedia {
  set onCandidates(void Function(List<String> candidates)? f);
  set onState(void Function(MemCallState state)? f);
  set onPath(void Function(MemCallPath path)? f);
  Future<String> offer();
  Future<String> restartOffer();
  Future<String> answer(String offerSdp);
  Future<String?> answerRestart(String offerSdp, {required bool polite});
  Future<void> answered(String answerSdp);
  Future<void> addCandidates(List<String> lines);
  void mute({required bool muted});
  Future<void> speaker({required bool on});
  Future<void> close();
}

/// Everything a call does outside itself. The bloc decides; these only carry it out.
abstract interface class CallEffects {
  /// Calls are on (supported here and allowed in settings).
  bool get enabled;
  CallLimits get limits;

  /// Opens the microphone and prepares the media.
  Future<CallMedia> openMedia();

  /// Sends call setup to the contact; throws when it cannot.
  Future<void> send(String contactId, CallSignalItem signal);

  /// Plays [sound] (for the ringtone, the one [contactId] has); replaces any sound playing.
  void play(CallSound sound, {required String contactId});
  void stopSound();

  /// Keeps the call in the chat's history (when the user keeps it).
  Future<void> record(CallRecord record);

  /// Shows the call screen.
  void showScreen();

  /// Releases the sound player.
  Future<void> dispose();
}

/// What happens to a call. Public events are the user's; the rest come from the bloc's own
/// media and timers.
sealed class CallEvent {
  const CallEvent();
}

/// The user calls [contactId]; [result] gets `null` once it rings out, or a reason key.
final class CallStartRequested extends CallEvent {
  CallStartRequested(this.contactId, this.result);
  final String contactId;
  final Completer<String?> result;
}

/// The user accepts the ringing call; [micGranted] whether the microphone may be used.
final class CallAcceptRequested extends CallEvent {
  const CallAcceptRequested({required this.micGranted});
  final bool micGranted;
}

final class CallDeclineRequested extends CallEvent {
  const CallDeclineRequested();
}

final class CallHangUpRequested extends CallEvent {
  const CallHangUpRequested();
}

final class CallMuteToggled extends CallEvent {
  const CallMuteToggled();
}

final class CallSpeakerToggled extends CallEvent {
  const CallSpeakerToggled();
}

/// Call setup from a contact arrived.
final class CallSignalReceived extends CallEvent {
  const CallSignalReceived(this.signal);
  final IncomingCallItem signal;
}

final class _MediaStateChanged extends CallEvent {
  const _MediaStateChanged(this.callId, this.state);
  final String callId;
  final MemCallState state;
}

final class _MediaPathChanged extends CallEvent {
  const _MediaPathChanged(this.callId, this.path);
  final String callId;
  final MemCallPath path;
}

final class _LocalCandidates extends CallEvent {
  const _LocalCandidates(this.callId, this.lines);
  final String callId;
  final List<String> lines;
}

enum _Limit { reach, ring, connect, drop, restartFirst, restart }

final class _LimitReached extends CallEvent {
  const _LimitReached(this.callId, this.limit);
  final String callId;
  final _Limit limit;
}

final class _EndShown extends CallEvent {
  const _EndShown(this.callId);
  final String callId;
}

/// One call at a time, as a state machine. Every event is handled to the end before the next
/// starts ([sequential]), so a fast answer, a timer and a tap can never interleave.
class CallBloc extends Bloc<CallEvent, CallState> {
  CallBloc(
    this.effects, {
    DateTime Function()? now,
    String Function()? newCallId,
  }) : _now = now ?? DateTime.now,
       _newCallId = newCallId ?? _randomCallId,
       super(const CallState()) {
    on<CallEvent>(_on, transformer: sequential());
  }

  final CallEffects effects;
  final DateTime Function() _now;
  final String Function() _newCallId;

  /// How long the end of a call stays on the screen.
  static const endShownFor = Duration(seconds: 2);

  CallMedia? _media;

  /// The caller's description, kept until the user accepts.
  String? _offerSdp;

  /// Their candidates, kept until our media is ready for them.
  final _pending = <String>[];
  final _timers = <_Limit, Timer>{};

  /// Clears an ended call from the screen.
  Timer? _shown;

  static String _randomCallId() {
    final r = Random.secure();
    return [for (var i = 0; i < 16; i++) r.nextInt(256)]
        .map((b) => b.toRadixString(16).padLeft(2, '0'))
        .join();
  }

  ActiveCall? get _call => state.call;

  Future<void> _on(CallEvent e, Emitter<CallState> emit) => switch (e) {
    CallStartRequested() => _start(e, emit),
    CallAcceptRequested() => _accept(e, emit),
    CallDeclineRequested() => _finish(CallEndReason.declined, emit),
    CallHangUpRequested() => _hangUp(emit),
    CallMuteToggled() => _toggleMute(emit),
    CallSpeakerToggled() => _toggleSpeaker(emit),
    CallSignalReceived() => _signal(e.signal, emit),
    _MediaStateChanged() => _mediaState(e, emit),
    _MediaPathChanged() => _mediaPath(e, emit),
    _LocalCandidates() => _localCandidates(e),
    _LimitReached() => _limit(e, emit),
    _EndShown() => _endShown(e, emit),
  };

  void _set(ActiveCall? c, Emitter<CallState> emit) => emit(CallState(c));

  Future<void> _start(CallStartRequested e, Emitter<CallState> emit) async {
    if (!effects.enabled) return e.result.complete('unsupported');
    if (_call != null) return e.result.complete('busy');
    final c = ActiveCall(
      contactId: e.contactId,
      callId: _newCallId(),
      outgoing: true,
      phase: CallPhase.dialing,
      started: _now(),
    );
    _set(c, emit);
    _arm(c, _Limit.reach, effects.limits.reachFor);
    effects.play(CallSound.dial, contactId: c.contactId);
    try {
      final media = await _open(c);
      final sdp = await media.offer();
      await effects.send(
        c.contactId,
        CallSignalItem.offer(callId: c.callId, sdp: sdp),
      );
    } on Object catch (err) {
      await _finish(CallEndReason.failed, emit, tell: false);
      return e.result.complete(err.toString());
    }
    effects.showScreen();
    e.result.complete(null);
  }

  Future<void> _accept(CallAcceptRequested e, Emitter<CallState> emit) async {
    final c = _call;
    if (c == null || c.phase != CallPhase.ringing) return;
    if (!e.micGranted) return _finish(CallEndReason.declined, emit);
    _cancel(_Limit.ring);
    effects.stopSound();
    final connecting = c.copyWith(phase: CallPhase.connecting);
    _set(connecting, emit);
    _arm(connecting, _Limit.connect, effects.limits.connectFor);
    try {
      final media = await _open(connecting);
      final sdp = await media.answer(_offerSdp ?? '');
      await effects.send(
        c.contactId,
        CallSignalItem.answer(callId: c.callId, sdp: sdp),
      );
      await media.addCandidates(List.of(_pending));
      _pending.clear();
    } on Object {
      await _finish(CallEndReason.failed, emit);
    }
  }

  Future<void> _hangUp(Emitter<CallState> emit) {
    final phase = _call?.phase;
    return _finish(
      phase == CallPhase.calling || phase == CallPhase.dialing
          ? CallEndReason.unanswered
          : CallEndReason.hungUp,
      emit,
    );
  }

  Future<void> _toggleMute(Emitter<CallState> emit) async {
    final c = _call;
    if (c == null || c.phase == CallPhase.ended) return;
    final next = c.copyWith(muted: !c.muted);
    _media?.mute(muted: next.muted);
    _set(next, emit);
  }

  Future<void> _toggleSpeaker(Emitter<CallState> emit) async {
    final c = _call;
    if (c == null || c.phase == CallPhase.ended) return;
    final next = c.copyWith(speaker: !c.speaker);
    _set(next, emit);
    try {
      await _media?.speaker(on: next.speaker);
    } on Object {
      // The route stays as it was; the next connect applies the choice again.
    }
  }

  bool _ours(ActiveCall? c, String callId, String contactId) =>
      c != null &&
      c.phase != CallPhase.ended &&
      c.callId == callId &&
      c.contactId == contactId;

  Future<void> _signal(IncomingCallItem s, Emitter<CallState> emit) async {
    final c = _call;
    switch (s.signal) {
      case CallSignalItem_Offer(:final callId, :final sdp):
        if (!effects.enabled) return;
        if (c != null) {
          // The other side restarts the media path of the call in progress: answer it.
          if (_ours(c, callId, s.contactId) && c.talkingSince != null) {
            try {
              final answer = await _media?.answerRestart(
                sdp,
                polite: !c.outgoing,
              );
              if (answer != null) {
                await _trySend(
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
            await _trySend(
              s.contactId,
              CallSignalItem.end(callId: callId, reason: CallEndReason.busy),
            );
          }
          return;
        }
        final incoming = ActiveCall(
          contactId: s.contactId,
          callId: callId,
          outgoing: false,
          phase: CallPhase.ringing,
          started: _now(),
        );
        _offerSdp = sdp;
        _pending.clear();
        _set(incoming, emit);
        _arm(incoming, _Limit.ring, effects.limits.ringFor);
        effects
          ..play(CallSound.ringtone, contactId: s.contactId)
          ..showScreen();
        // Tell the caller it rings here, so it plays its ringback only now.
        await _trySend(s.contactId, CallSignalItem.ringing(callId: callId));
      case CallSignalItem_Ringing(:final callId):
        if (!_ours(c, callId, s.contactId) ||
            !c!.outgoing ||
            c.phase != CallPhase.dialing) {
          return;
        }
        final calling = c.copyWith(reached: true, phase: CallPhase.calling);
        _set(calling, emit);
        _cancel(_Limit.reach);
        _arm(calling, _Limit.ring, effects.limits.ringFor);
        effects.play(CallSound.ringback, contactId: c.contactId);
      case CallSignalItem_Answer(:final callId, :final sdp):
        if (!_ours(c, callId, s.contactId)) return;
        // The answer to a restart in a call in progress (either side): just apply it.
        if (c!.talkingSince != null) {
          try {
            await _media?.answered(sdp);
          } on Object {
            // A stale answer; the next restart sends a new offer.
          }
          return;
        }
        if (!c.outgoing ||
            (c.phase != CallPhase.dialing && c.phase != CallPhase.calling)) {
          return;
        }
        // An answer also proves it got there (an older app sends no "ringing").
        final connecting = c.copyWith(
          reached: true,
          phase: CallPhase.connecting,
        );
        _cancel(_Limit.reach);
        _cancel(_Limit.ring);
        effects.stopSound();
        _set(connecting, emit);
        _arm(connecting, _Limit.connect, effects.limits.connectFor);
        try {
          await _media?.answered(sdp);
        } on Object {
          await _finish(CallEndReason.failed, emit);
        }
      case CallSignalItem_Candidates(:final callId, :final candidates):
        if (!_ours(c, callId, s.contactId)) return;
        final media = _media;
        if (media == null) {
          _pending.addAll(candidates);
        } else {
          try {
            await media.addCandidates(candidates);
          } on Object {
            // A candidate the media refuses; others may still connect.
          }
        }
      case CallSignalItem_End(:final callId, :final reason):
        if (!_ours(c, callId, s.contactId)) return;
        await _finish(reason, emit, tell: false);
    }
  }

  Future<CallMedia> _open(ActiveCall c) async {
    final media = await effects.openMedia();
    _media = media;
    final id = c.callId;
    media
      ..onCandidates = ((lines) => _addIfOpen(_LocalCandidates(id, lines)))
      ..onPath = ((path) => _addIfOpen(_MediaPathChanged(id, path)))
      ..onState = ((s) => _addIfOpen(_MediaStateChanged(id, s)));
    // A new call starts on the earpiece unless the user already chose the loudspeaker (it may
    // have been pressed while the call was still ringing).
    unawaited(media.speaker(on: c.speaker).catchError((Object _) {}));
    return media;
  }

  void _addIfOpen(CallEvent e) {
    if (!isClosed) add(e);
  }

  Future<void> _mediaState(
    _MediaStateChanged e,
    Emitter<CallState> emit,
  ) async {
    final c = _call;
    if (c == null || c.callId != e.callId || c.phase == CallPhase.ended) {
      return;
    }
    switch (e.state) {
      case MemCallState.connected:
        _cancel(_Limit.connect);
        _cancel(_Limit.drop);
        _cancel(_Limit.restartFirst);
        _cancel(_Limit.restart);
        _set(
          c.copyWith(
            phase: CallPhase.talking,
            reconnecting: false,
            talkingSince: c.talkingSince ?? _now(),
          ),
          emit,
        );
        // Android picks its default route when audio starts or comes back: apply the
        // user's choice again so the button and the sound agree.
        try {
          await _media?.speaker(on: c.speaker);
        } on Object {
          // The route stays as it was.
        }
      case MemCallState.interrupted || MemCallState.failed
          when c.talkingSince != null:
        // In a call: never end on a hiccup. Wait for the media to come back; only a long
        // loss ends it (the user can end it any time).
        if (c.reconnecting) return;
        final waiting = c.copyWith(reconnecting: true);
        _set(waiting, emit);
        _arm(waiting, _Limit.drop, effects.limits.dropAfter);
        // Both sides ask for a fresh path while cut off (a lost path does not come back by
        // itself once WebRTC has given it up); the callee a little later, so that the two
        // rarely offer at once.
        _arm(
          waiting,
          _Limit.restartFirst,
          Duration(seconds: c.outgoing ? 3 : 9),
        );
      case MemCallState.failed:
        // Never connected: it cannot.
        await _finish(CallEndReason.failed, emit);
      case MemCallState.interrupted ||
          MemCallState.connecting ||
          MemCallState.closed:
        break;
    }
  }

  Future<void> _mediaPath(_MediaPathChanged e, Emitter<CallState> emit) async {
    final c = _call;
    if (c == null || c.callId != e.callId || c.phase == CallPhase.ended) {
      return;
    }
    _set(c.copyWith(path: e.path), emit);
  }

  Future<void> _localCandidates(_LocalCandidates e) async {
    final c = _call;
    if (c == null || c.callId != e.callId || c.phase == CallPhase.ended) {
      return;
    }
    await _trySend(
      c.contactId,
      CallSignalItem.candidates(callId: c.callId, candidates: e.lines),
    );
  }

  Future<void> _limit(_LimitReached e, Emitter<CallState> emit) async {
    final c = _call;
    if (c == null || c.callId != e.callId || c.phase == CallPhase.ended) {
      return;
    }
    switch (e.limit) {
      case _Limit.reach:
        if (!c.reached) await _finish(CallEndReason.unanswered, emit);
      case _Limit.ring:
        if (c.phase == CallPhase.ringing || c.phase == CallPhase.calling) {
          await _finish(CallEndReason.unanswered, emit);
        }
      case _Limit.connect:
        if (c.talkingSince == null) await _finish(CallEndReason.failed, emit);
      case _Limit.drop:
        if (c.reconnecting) await _finish(CallEndReason.failed, emit);
      case _Limit.restartFirst || _Limit.restart:
        if (!c.reconnecting) return;
        if (e.limit == _Limit.restartFirst) {
          final every = effects.limits.restartEvery;
          final id = c.callId;
          _timers[_Limit.restart]?.cancel();
          _timers[_Limit.restart] = Timer.periodic(
            every,
            (_) => _addIfOpen(_LimitReached(id, _Limit.restart)),
          );
        }
        final media = _media;
        if (media == null) return;
        try {
          final sdp = await media.restartOffer();
          await effects.send(
            c.contactId,
            CallSignalItem.offer(callId: c.callId, sdp: sdp),
          );
        } on Object {
          // No channel right now: the next try may find one.
        }
    }
  }

  void _arm(ActiveCall c, _Limit limit, Duration after) {
    _timers[limit]?.cancel();
    final id = c.callId;
    _timers[limit] = Timer(after, () => _addIfOpen(_LimitReached(id, limit)));
  }

  void _cancel(_Limit limit) => _timers.remove(limit)?.cancel();

  void _cancelAll() {
    for (final t in _timers.values) {
      t.cancel();
    }
    _timers.clear();
  }

  Future<void> _trySend(String contactId, CallSignalItem signal) async {
    try {
      await effects.send(contactId, signal);
    } on Object {
      // The other side's own timers end the call if this does not get through.
    }
  }

  /// Ends the call for [reason]; tells the other side unless it told us.
  Future<void> _finish(
    CallEndReason reason,
    Emitter<CallState> emit, {
    bool tell = true,
  }) async {
    final c = _call;
    if (c == null || c.phase == CallPhase.ended) return;
    _cancelAll();
    effects.stopSound();
    final ended = c.copyWith(phase: CallPhase.ended, endReason: reason);
    _set(ended, emit);
    if (tell) {
      await _trySend(
        c.contactId,
        CallSignalItem.end(callId: c.callId, reason: reason),
      );
    }
    final media = _media;
    _media = null;
    _offerSdp = null;
    _pending.clear();
    // The loudspeaker is a phone-wide setting: never leave it on for the next call.
    if (c.speaker) {
      try {
        await media?.speaker(on: false);
      } on Object {
        // Nothing to switch back.
      }
    }
    // A cue when the call did not happen or broke off, not when someone just hung up.
    final broke =
        (c.outgoing && !c.reached) ||
        reason == CallEndReason.failed ||
        (!tell && reason != CallEndReason.hungUp);
    if (broke) effects.play(CallSound.unavailable, contactId: c.contactId);
    try {
      await media?.close();
    } on Object {
      // Already closed.
    }
    await _record(ended);
    // The screen shows the ending for a moment, then the call is gone.
    final id = c.callId;
    _shown?.cancel();
    _shown = Timer(endShownFor, () => _addIfOpen(_EndShown(id)));
  }

  Future<void> _endShown(_EndShown e, Emitter<CallState> emit) async {
    final c = _call;
    if (c != null && c.callId == e.callId && c.phase == CallPhase.ended) {
      _set(null, emit);
    }
  }

  Future<void> _record(ActiveCall c) async {
    final talked = c.talkingSince;
    final reason = c.endReason ?? CallEndReason.hungUp;
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
      await effects.record(
        CallRecord(
          contactId: c.contactId,
          outgoing: c.outgoing,
          started: c.started,
          durationS: talked == null ? 0 : _now().difference(talked).inSeconds,
          outcome: outcome,
          via: talked == null
              ? null
              : switch (c.path) {
                  MemCallPath.wifiDirect => Channel.wifiDirect,
                  // Direct between the devices, no server: the grade of the local network.
                  MemCallPath.localNetwork || MemCallPath.direct => Channel.lan,
                  MemCallPath.servers => Channel.hub,
                  null => null,
                },
        ),
      );
    } on Object {
      // History is a convenience; a failure must not disturb the call.
    }
  }

  @override
  Future<void> close() async {
    _cancelAll();
    _shown?.cancel();
    final media = _media;
    _media = null;
    try {
      await media?.close();
    } on Object {
      // Already closed.
    }
    await effects.dispose();
    return super.close();
  }
}
