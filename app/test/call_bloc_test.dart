import 'dart:async';

import 'package:bloc_test/bloc_test.dart';
import 'package:fake_async/fake_async.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mem/calls/call_bloc.dart';
import 'package:mem/src/rust/api.dart';
import 'package:mem_calls/mem_calls.dart';

const _contact = 'aa';
const _other = 'bb';
const _id = 'c1';

const _limits = CallLimits(
  ringFor: Duration(seconds: 45),
  reachFor: Duration(seconds: 20),
  dropAfter: Duration(seconds: 60),
);

class FakeMedia implements CallMedia {
  void Function(List<String>)? candidates;
  void Function(MemCallState)? state;
  void Function(MemCallPath)? path;

  /// Completes the offer; tests may hold it back to let other events queue up.
  Completer<String>? holdOffer;
  final log = <String>[];
  final added = <String>[];
  final speakers = <bool>[];
  bool closed = false;

  @override
  set onCandidates(void Function(List<String>)? f) => candidates = f;
  @override
  set onState(void Function(MemCallState)? f) => state = f;
  @override
  set onPath(void Function(MemCallPath)? f) => path = f;
  @override
  Future<String> offer() async {
    log.add('offer');
    return holdOffer?.future ?? 'sdp-offer';
  }

  @override
  Future<String> restartOffer() async {
    log.add('restartOffer');
    return 'sdp-restart';
  }

  @override
  Future<String> answer(String offerSdp) async {
    log.add('answer $offerSdp');
    return 'sdp-answer';
  }

  @override
  Future<String?> answerRestart(String offerSdp, {required bool polite}) async {
    log.add('answerRestart polite=$polite');
    return 'sdp-restart-answer';
  }

  @override
  Future<void> answered(String answerSdp) async => log.add('answered');
  @override
  Future<void> addCandidates(List<String> lines) async => added.addAll(lines);
  @override
  void mute({required bool muted}) => log.add('mute $muted');
  @override
  Future<void> speaker({required bool on}) async => speakers.add(on);
  @override
  Future<void> close() async => closed = true;
}

class FakeEffects implements CallEffects {
  final media = <FakeMedia>[];
  final sent = <(String, CallSignalItem)>[];
  final sounds = <String>[];
  final records = <CallRecord>[];
  var screens = 0;
  var failSend = false;
  Completer<String>? holdOffer;

  FakeMedia get last => media.last;

  @override
  bool enabled = true;
  @override
  CallLimits get limits => _limits;
  @override
  Future<CallMedia> openMedia() async {
    final m = FakeMedia()..holdOffer = holdOffer;
    media.add(m);
    return m;
  }

  @override
  Future<void> send(String contactId, CallSignalItem signal) async {
    if (failSend) throw StateError('no channel');
    sent.add((contactId, signal));
  }

  @override
  void play(CallSound sound, {required String contactId}) =>
      sounds.add(sound.name);
  @override
  void stopSound() => sounds.add('stop');
  @override
  Future<void> record(CallRecord record) async => records.add(record);
  @override
  void showScreen() => screens++;
  @override
  Future<void> dispose() async {}

  /// The kinds of what was sent, in order (`offer`, `ringing`, `end:busy` …).
  List<String> get kinds => [
    for (final (_, s) in sent)
      switch (s) {
        CallSignalItem_Offer() => 'offer',
        CallSignalItem_Answer() => 'answer',
        CallSignalItem_Ringing() => 'ringing',
        CallSignalItem_Candidates() => 'candidates',
        CallSignalItem_End(:final reason) => 'end:${reason.name}',
      },
  ];
}

IncomingCallItem _in(CallSignalItem s, {String from = _contact}) =>
    IncomingCallItem(
      contactId: from,
      signal: s,
      via: const ChannelUseItem(channel: Channel.lan, grade: 'P1', atUnixMs: 0),
    );

/// Runs [body] with fake time, a bloc and its fakes.
void _run(void Function(FakeAsync a, CallBloc b, FakeEffects fx) body) {
  fakeAsync((a) {
    final fx = FakeEffects();
    final clock = a.getClock(DateTime(2026, 11, 14, 12));
    final b = CallBloc(fx, now: clock.now, newCallId: () => _id);
    body(a, b, fx);
    unawaited(b.close());
    a.flushMicrotasks();
  });
}

/// Starts an outgoing call to [_contact] and returns its result future's value holder.
List<String?> _start(FakeAsync a, CallBloc b) {
  final result = <String?>[];
  final done = Completer<String?>();
  unawaited(done.future.then(result.add));
  b.add(CallStartRequested(_contact, done));
  a.flushMicrotasks();
  return result;
}

/// An incoming call from [_contact], accepted, with its media connected.
void _talkingIncoming(FakeAsync a, CallBloc b, FakeEffects fx) {
  b
    ..add(
      CallSignalReceived(
        _in(const CallSignalItem.offer(callId: _id, sdp: 'o')),
      ),
    )
    ..add(const CallAcceptRequested(micGranted: true));
  a.flushMicrotasks();
  fx.last.state!(MemCallState.connected);
  a.flushMicrotasks();
}

/// An outgoing call that rang, was answered and connected.
void _talkingOutgoing(FakeAsync a, CallBloc b, FakeEffects fx) {
  _start(a, b);
  b
    ..add(CallSignalReceived(_in(const CallSignalItem.ringing(callId: _id))))
    ..add(
      CallSignalReceived(
        _in(const CallSignalItem.answer(callId: _id, sdp: 'a')),
      ),
    );
  a.flushMicrotasks();
  fx.last.state!(MemCallState.connected);
  a.flushMicrotasks();
}

void main() {
  group('outgoing', () {
    test('dial, ringback, answer, talk, hang up', () {
      _run((a, b, fx) {
        final result = _start(a, b);
        expect(result, [null]);
        expect(b.state.call!.phase, CallPhase.dialing);
        expect(fx.kinds, ['offer']);
        expect(fx.screens, 1);
        b.add(
          CallSignalReceived(_in(const CallSignalItem.ringing(callId: _id))),
        );
        a.flushMicrotasks();
        expect(b.state.call!.phase, CallPhase.calling);
        expect(fx.sounds, ['dial', 'ringback']);
        b.add(
          CallSignalReceived(
            _in(const CallSignalItem.answer(callId: _id, sdp: 'a')),
          ),
        );
        a.flushMicrotasks();
        expect(b.state.call!.phase, CallPhase.connecting);
        expect(fx.sounds.last, 'stop');
        fx.last
          ..path!(MemCallPath.localNetwork)
          ..state!(MemCallState.connected);
        a
          ..flushMicrotasks()
          ..elapse(const Duration(seconds: 30));
        expect(b.state.call!.phase, CallPhase.talking);
        b.add(const CallHangUpRequested());
        a.flushMicrotasks();
        expect(b.state.call!.phase, CallPhase.ended);
        expect(fx.kinds.last, 'end:hungUp');
        expect(fx.records.single.outcome, CallOutcomeItem.answered);
        expect(fx.records.single.via, Channel.lan);
        expect(fx.records.single.durationS, 30);
        // Someone hung up: no "unavailable" cue.
        expect(fx.sounds, isNot(contains('unavailable')));
        expect(fx.last.closed, isTrue);
        a.elapse(CallBloc.endShownFor);
        expect(b.state.call, isNull);
      });
    });

    test('a ringing that arrives while the offer is still being made wins', () {
      _run((a, b, fx) {
        fx.holdOffer = Completer<String>();
        _start(a, b);
        // Queued behind the start, not interleaved with it.
        b.add(
          CallSignalReceived(_in(const CallSignalItem.ringing(callId: _id))),
        );
        a.flushMicrotasks();
        expect(b.state.call!.phase, CallPhase.dialing);
        fx.holdOffer!.complete('sdp');
        a.flushMicrotasks();
        expect(fx.kinds, ['offer']);
        expect(b.state.call!.phase, CallPhase.calling);
        expect(fx.sounds, ['dial', 'ringback']);
      });
    });

    test('candidates gathered during the offer go out after it', () {
      _run((a, b, fx) {
        fx.holdOffer = Completer<String>();
        _start(a, b);
        fx.last.candidates!(['cand1']);
        fx.holdOffer!.complete('sdp');
        a.flushMicrotasks();
        expect(fx.kinds, ['offer', 'candidates']);
      });
    });

    test('an answer without ringing (older app) still counts as reached', () {
      _run((a, b, fx) {
        _start(a, b);
        b.add(
          CallSignalReceived(
            _in(const CallSignalItem.answer(callId: _id, sdp: 'a')),
          ),
        );
        a.flushMicrotasks();
        expect(b.state.call!.reached, isTrue);
        expect(b.state.call!.phase, CallPhase.connecting);
        // The reach limit no longer applies.
        a.elapse(const Duration(seconds: 25));
        expect(b.state.call!.phase, CallPhase.connecting);
      });
    });

    test('no ringing within the reach limit: not reachable', () {
      _run((a, b, fx) {
        _start(a, b);
        a.elapse(const Duration(seconds: 19));
        expect(b.state.call!.phase, CallPhase.dialing);
        a.elapse(const Duration(seconds: 1));
        expect(b.state.call!.endReason, CallEndReason.unanswered);
        expect(fx.kinds.last, 'end:unanswered');
        expect(fx.records.single.outcome, CallOutcomeItem.unreachable);
        expect(fx.sounds.last, 'unavailable');
      });
    });

    test('ringing replaces the reach limit with the ring limit', () {
      _run((a, b, fx) {
        _start(a, b);
        a.elapse(const Duration(seconds: 5));
        b.add(
          CallSignalReceived(_in(const CallSignalItem.ringing(callId: _id))),
        );
        a.elapse(const Duration(seconds: 44));
        expect(b.state.call!.phase, CallPhase.calling);
        a.elapse(const Duration(seconds: 1));
        expect(b.state.call!.phase, CallPhase.ended);
        expect(fx.records.single.outcome, CallOutcomeItem.unanswered);
        expect(fx.sounds, isNot(contains('unavailable')));
      });
    });

    test('declined and busy end the call with the reason', () {
      for (final reason in [CallEndReason.declined, CallEndReason.busy]) {
        _run((a, b, fx) {
          _start(a, b);
          b.add(
            CallSignalReceived(
              _in(CallSignalItem.end(callId: _id, reason: reason)),
            ),
          );
          a.flushMicrotasks();
          expect(b.state.call!.endReason, reason);
          // They told us: nothing is sent back.
          expect(fx.kinds, ['offer']);
          expect(fx.records.single.outcome, CallOutcomeItem.unreachable);
        });
      }
    });

    test('an offer that cannot be sent fails at once', () {
      _run((a, b, fx) {
        fx.failSend = true;
        final result = _start(a, b);
        expect(result.single, contains('no channel'));
        expect(b.state.call!.phase, CallPhase.ended);
        expect(fx.screens, 0);
        a.elapse(CallBloc.endShownFor);
        expect(b.state.call, isNull);
      });
    });

    test('a second call while one is in progress is busy', () {
      _run((a, b, fx) {
        _start(a, b);
        final done = Completer<String?>();
        b.add(CallStartRequested(_other, done));
        a.flushMicrotasks();
        expect(done.isCompleted, isTrue);
        unawaited(done.future.then((r) => expect(r, 'busy')));
        a.flushMicrotasks();
      });
    });

    test('an answered call whose media never connects fails after 45 s', () {
      _run((a, b, fx) {
        _start(a, b);
        b.add(
          CallSignalReceived(
            _in(const CallSignalItem.answer(callId: _id, sdp: 'a')),
          ),
        );
        a.elapse(const Duration(seconds: 44));
        expect(b.state.call!.phase, CallPhase.connecting);
        a.elapse(const Duration(seconds: 1));
        expect(b.state.call!.endReason, CallEndReason.failed);
        expect(fx.records.single.outcome, CallOutcomeItem.failed);
      });
    });
  });

  group('incoming', () {
    test('rings, tells the caller, accepts with the waiting candidates', () {
      _run((a, b, fx) {
        b
          ..add(
            CallSignalReceived(
              _in(const CallSignalItem.offer(callId: _id, sdp: 'o')),
            ),
          )
          ..add(
            CallSignalReceived(
              _in(
                const CallSignalItem.candidates(callId: _id, candidates: ['x']),
              ),
            ),
          );
        a.flushMicrotasks();
        expect(b.state.call!.phase, CallPhase.ringing);
        expect(fx.sounds, ['ringtone']);
        expect(fx.kinds, ['ringing']);
        expect(fx.screens, 1);
        b.add(const CallAcceptRequested(micGranted: true));
        a.flushMicrotasks();
        expect(b.state.call!.phase, CallPhase.connecting);
        expect(fx.last.log, ['answer o']);
        expect(fx.last.added, ['x']);
        expect(fx.kinds, ['ringing', 'answer']);
      });
    });

    test('declined, or accepted without the microphone', () {
      for (final ev in <CallEvent>[
        const CallDeclineRequested(),
        const CallAcceptRequested(micGranted: false),
      ]) {
        _run((a, b, fx) {
          b
            ..add(
              CallSignalReceived(
                _in(const CallSignalItem.offer(callId: _id, sdp: 'o')),
              ),
            )
            ..add(ev);
          a.flushMicrotasks();
          expect(b.state.call!.endReason, CallEndReason.declined);
          expect(fx.kinds.last, 'end:declined');
          expect(fx.records.single.outcome, CallOutcomeItem.declined);
          expect(fx.media, isEmpty);
        });
      }
    });

    test('unanswered within the ring limit: missed', () {
      _run((a, b, fx) {
        b.add(
          CallSignalReceived(
            _in(const CallSignalItem.offer(callId: _id, sdp: 'o')),
          ),
        );
        a.elapse(const Duration(seconds: 45));
        expect(b.state.call!.endReason, CallEndReason.unanswered);
        expect(fx.records.single.outcome, CallOutcomeItem.missed);
      });
    });

    test('the caller stops before an answer: missed, with the cue', () {
      _run((a, b, fx) {
        b
          ..add(
            CallSignalReceived(
              _in(const CallSignalItem.offer(callId: _id, sdp: 'o')),
            ),
          )
          ..add(
            CallSignalReceived(
              _in(
                const CallSignalItem.end(
                  callId: _id,
                  reason: CallEndReason.unanswered,
                ),
              ),
            ),
          );
        a.flushMicrotasks();
        expect(fx.records.single.outcome, CallOutcomeItem.missed);
        expect(fx.sounds.last, 'unavailable');
      });
    });

    test('an end for another call, or from another contact, is ignored', () {
      _run((a, b, fx) {
        b
          ..add(
            CallSignalReceived(
              _in(const CallSignalItem.offer(callId: _id, sdp: 'o')),
            ),
          )
          ..add(
            CallSignalReceived(
              _in(
                const CallSignalItem.end(
                  callId: _id,
                  reason: CallEndReason.hungUp,
                ),
                from: _other,
              ),
            ),
          );
        a.flushMicrotasks();
        expect(b.state.call!.phase, CallPhase.ringing);
      });
    });

    blocTest<CallBloc, CallState>(
      'another offer during a call is answered busy and changes nothing',
      build: () => CallBloc(FakeEffects(), newCallId: () => _id),
      act: (b) => b
        ..add(
          CallSignalReceived(
            _in(const CallSignalItem.offer(callId: _id, sdp: 'o')),
          ),
        )
        ..add(
          CallSignalReceived(
            _in(
              const CallSignalItem.offer(callId: 'c2', sdp: 'o'),
              from: _other,
            ),
          ),
        ),
      verify: (b) {
        final fx = b.effects as FakeEffects;
        expect(fx.sent.last.$1, _other);
        expect(fx.kinds, ['ringing', 'end:busy']);
        expect(b.state.call!.contactId, _contact);
      },
    );

    blocTest<CallBloc, CallState>(
      'offers are ignored while calls are off',
      build: () => CallBloc(FakeEffects()..enabled = false),
      act: (b) => b.add(
        CallSignalReceived(
          _in(const CallSignalItem.offer(callId: _id, sdp: 'o')),
        ),
      ),
      expect: () => <CallState>[],
    );
  });

  group('in a call', () {
    test(
      'the media drops and comes back: reconnecting, one restart, talking',
      () {
        _run((a, b, fx) {
          _talkingOutgoing(a, b, fx);
          final media = fx.last;
          media.state!(MemCallState.interrupted);
          a.flushMicrotasks();
          expect(b.state.call!.reconnecting, isTrue);
          expect(b.state.call!.phase, CallPhase.talking);
          // The caller asks for a fresh path after 3 s.
          a.elapse(const Duration(seconds: 3));
          expect(media.log.where((l) => l == 'restartOffer'), hasLength(1));
          expect(fx.kinds.last, 'offer');
          media.state!(MemCallState.connected);
          a.flushMicrotasks();
          expect(b.state.call!.reconnecting, isFalse);
          // No more restarts, and the drop limit is gone.
          a.elapse(const Duration(seconds: 120));
          expect(media.log.where((l) => l == 'restartOffer'), hasLength(1));
          expect(b.state.call!.phase, CallPhase.talking);
        });
      },
    );

    test('retries every 15 s while lost, ends after the drop limit', () {
      _run((a, b, fx) {
        _talkingOutgoing(a, b, fx);
        fx.last.state!(MemCallState.failed);
        a.elapse(const Duration(seconds: 59));
        // At 3, 18, 33 and 48 s.
        expect(fx.last.log.where((l) => l == 'restartOffer'), hasLength(4));
        expect(b.state.call!.phase, CallPhase.talking);
        a.elapse(const Duration(seconds: 1));
        expect(b.state.call!.endReason, CallEndReason.failed);
        expect(fx.sounds.last, 'unavailable');
        expect(fx.records.single.outcome, CallOutcomeItem.answered);
      });
    });

    test('the callee waits longer before its own restart', () {
      _run((a, b, fx) {
        _talkingIncoming(a, b, fx);
        fx.last.state!(MemCallState.interrupted);
        a.elapse(const Duration(seconds: 8));
        expect(fx.last.log, isNot(contains('restartOffer')));
        a.elapse(const Duration(seconds: 1));
        expect(fx.last.log, contains('restartOffer'));
      });
    });

    test('a restart offer from the other side is answered (callee polite)', () {
      _run((a, b, fx) {
        _talkingIncoming(a, b, fx);
        b.add(
          CallSignalReceived(
            _in(const CallSignalItem.offer(callId: _id, sdp: 'r')),
          ),
        );
        a.flushMicrotasks();
        expect(fx.last.log.last, 'answerRestart polite=true');
        expect(fx.kinds.last, 'answer');
        expect(b.state.call!.phase, CallPhase.talking);
      });
    });

    test('both sides restart at once: the caller is not polite', () {
      _run((a, b, fx) {
        _talkingOutgoing(a, b, fx);
        b.add(
          CallSignalReceived(
            _in(const CallSignalItem.offer(callId: _id, sdp: 'r')),
          ),
        );
        a.flushMicrotasks();
        expect(fx.last.log.last, 'answerRestart polite=false');
      });
    });

    test('the answer to a restart is applied to the media', () {
      _run((a, b, fx) {
        _talkingOutgoing(a, b, fx);
        final before = fx.last.log.where((l) => l == 'answered').length;
        b.add(
          CallSignalReceived(
            _in(const CallSignalItem.answer(callId: _id, sdp: 'a2')),
          ),
        );
        a.flushMicrotasks();
        expect(
          fx.last.log.where((l) => l == 'answered'),
          hasLength(before + 1),
        );
        expect(b.state.call!.phase, CallPhase.talking);
      });
    });

    test(
      'the loudspeaker: kept from ringing, re-applied on connect, off at end',
      () {
        _run((a, b, fx) {
          b
            ..add(
              CallSignalReceived(
                _in(const CallSignalItem.offer(callId: _id, sdp: 'o')),
              ),
            )
            ..add(const CallSpeakerToggled())
            ..add(const CallAcceptRequested(micGranted: true));
          a.flushMicrotasks();
          final media = fx.last;
          expect(media.speakers, [true]);
          media.state!(MemCallState.connected);
          a.flushMicrotasks();
          expect(media.speakers, [true, true]);
          b.add(const CallMuteToggled());
          a.flushMicrotasks();
          expect(b.state.call!.muted, isTrue);
          expect(media.log.last, 'mute true');
          b.add(const CallHangUpRequested());
          a.flushMicrotasks();
          expect(media.speakers.last, isFalse);
        });
      },
    );
  });

  group('hang up', () {
    final cases =
        <
          String,
          (void Function(FakeAsync, CallBloc, FakeEffects), CallEndReason)
        >{
          'while dialing': (
            (a, b, fx) => _start(a, b),
            CallEndReason.unanswered,
          ),
          'while it rings there': (
            (a, b, fx) {
              _start(a, b);
              b.add(
                CallSignalReceived(
                  _in(const CallSignalItem.ringing(callId: _id)),
                ),
              );
              a.flushMicrotasks();
            },
            CallEndReason.unanswered,
          ),
          'while connecting': (
            (a, b, fx) {
              _start(a, b);
              b.add(
                CallSignalReceived(
                  _in(const CallSignalItem.answer(callId: _id, sdp: 'a')),
                ),
              );
              a.flushMicrotasks();
            },
            CallEndReason.hungUp,
          ),
          'while talking': (_talkingOutgoing, CallEndReason.hungUp),
          'while ringing here': (
            (a, b, fx) {
              b.add(
                CallSignalReceived(
                  _in(const CallSignalItem.offer(callId: _id, sdp: 'o')),
                ),
              );
              a.flushMicrotasks();
            },
            CallEndReason.hungUp,
          ),
        };
    for (final MapEntry(key: name, value: (setUp, reason)) in cases.entries) {
      test(name, () {
        _run((a, b, fx) {
          setUp(a, b, fx);
          b.add(const CallHangUpRequested());
          a.flushMicrotasks();
          expect(b.state.call!.endReason, reason);
          expect(fx.kinds.last, 'end:${reason.name}');
          expect(fx.sounds, contains('stop'));
          // A second tap changes nothing.
          b.add(const CallHangUpRequested());
          a.flushMicrotasks();
          expect(fx.kinds.where((k) => k.startsWith('end:')), hasLength(1));
          expect(fx.records, hasLength(1));
        });
      });
    }
  });
}
