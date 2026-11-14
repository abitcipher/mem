import 'dart:async';

import 'package:flutter/foundation.dart';

import 'package:material_ui/material_ui.dart';
import 'package:mem_calls/mem_calls.dart';
import 'package:mem_radio/mem_radio.dart';

import '../src/rust/api.dart';
import '../src/ui_settings.dart';
import 'call_bloc.dart';

export 'call_bloc.dart' show ActiveCall, CallPhase;

/// One call at a time, carried end to end over MEM: setup messages through the node, media
/// through [MemCallSession]. The decisions are [CallBloc]'s; this class gives the screens a
/// listenable view of it and carries out its effects.
class CallController extends ChangeNotifier {
  CallController(
    MemNode node,
    UiSettings settings,
    GlobalKey<NavigatorState> navigator,
    Route<void> Function() openScreen, {
    VoidCallback? onHistoryChanged,
  }) : _node = node,
       _settings = settings {
    bloc = CallBloc(
      _AppCallEffects(node, settings, navigator, openScreen, onHistoryChanged),
    );
    _sub = bloc.stream.listen((_) => notifyListeners());
  }

  final MemNode _node;
  final UiSettings _settings;

  /// The call state machine.
  late final CallBloc bloc;
  late final StreamSubscription<CallState> _sub;

  /// Whether calls can be made here at all (Android in this version; the setting decides
  /// whether they are on).
  static bool get supported => defaultTargetPlatform == TargetPlatform.android;

  bool get enabled => supported && _settings.voiceCalls;

  ActiveCall? get call => bloc.state.call;

  /// Calls [contactId]. Returns a reason key when it cannot start (`unsupported`, `busy`,
  /// `no_microphone`, or the error), or `null` once it is ringing out.
  Future<String?> start(String contactId) async {
    if (!enabled) return 'unsupported';
    if (call != null) return 'busy';
    if (!await MemRadio.soundRequestPermission()) return 'no_microphone';
    final result = Completer<String?>();
    bloc.add(CallStartRequested(contactId, result));
    return result.future;
  }

  /// Accepts the ringing call.
  Future<void> accept() async {
    if (call?.phase != CallPhase.ringing) return;
    final granted = await MemRadio.soundRequestPermission();
    bloc.add(CallAcceptRequested(micGranted: granted));
  }

  /// Declines the ringing call.
  void decline() => bloc.add(const CallDeclineRequested());

  /// Ends the call (or stops calling).
  void hangUp() => bloc.add(const CallHangUpRequested());

  void toggleMute() => bloc.add(const CallMuteToggled());

  void toggleSpeaker() => bloc.add(const CallSpeakerToggled());

  /// New setup messages arrived ([NodeEventItem_Call]).
  Future<void> onSignals() async {
    for (final s in _node.takeCallSignals()) {
      bloc.add(CallSignalReceived(s));
    }
  }

  @override
  void dispose() {
    unawaited(_sub.cancel());
    unawaited(bloc.close());
    super.dispose();
  }
}

/// The app's side of a call: the node, the settings, the sound player and the screen.
class _AppCallEffects implements CallEffects {
  _AppCallEffects(
    this.node,
    this.settings,
    this.navigator,
    this.openScreen,
    this.onHistoryChanged,
  );

  final MemNode node;
  final UiSettings settings;
  final GlobalKey<NavigatorState> navigator;
  final Route<void> Function() openScreen;
  final VoidCallback? onHistoryChanged;
  final _tones = MemTonePlayer();

  @override
  bool get enabled => CallController.supported && settings.voiceCalls;

  @override
  CallLimits get limits => CallLimits(
    // The user's settings, within their bounds; the caller cannot make our phone ring longer.
    ringFor: Duration(seconds: CallTimes.ring.clamp(settings.callRingSeconds)),
    reachFor: Duration(
      seconds: CallTimes.reach.clamp(settings.callReachSeconds),
    ),
    dropAfter: Duration(
      seconds: CallTimes.reconnect.clamp(settings.callReconnectSeconds),
    ),
  );

  @override
  Future<CallMedia> openMedia() async =>
      _SessionMedia(await MemCallSession.open());

  @override
  Future<void> send(String contactId, CallSignalItem signal) =>
      node.sendCallSignal(contactId: contactId, signal: signal);

  @override
  void play(CallSound sound, {required String contactId}) {
    final (String id, double volume, bool loop) = switch (sound) {
      CallSound.dial => (MemTone.dial.id, settings.outgoingVolume, true),
      CallSound.unavailable => (
        MemTone.unavailable.id,
        settings.outgoingVolume,
        false,
      ),
      CallSound.ringback => (
        settings.outgoingTone,
        settings.outgoingVolume,
        true,
      ),
      // The contact's own ringtone, else the default.
      CallSound.ringtone => (
        node.contacts().where((c) => c.id == contactId).firstOrNull?.ringtone ??
            settings.incomingTone,
        settings.incomingVolume,
        true,
      ),
    };
    final tone = MemTone.byId(id);
    // "off" plays nothing, but still stops what played before.
    if (tone == null) return stopSound();
    unawaited(
      _tones.play(tone, volume: volume, loop: loop).catchError((Object _) {}),
    );
  }

  @override
  void stopSound() => unawaited(_tones.stop().catchError((Object _) {}));

  @override
  Future<void> record(CallRecord r) async {
    if (!settings.callHistory) return;
    await node.recordCall(
      contactId: r.contactId,
      outgoing: r.outgoing,
      startedUnixMs: r.started.millisecondsSinceEpoch,
      durationS: r.durationS,
      outcome: r.outcome,
      via: r.via,
    );
    onHistoryChanged?.call();
  }

  @override
  void showScreen() => unawaited(navigator.currentState?.push(openScreen()));

  @override
  Future<void> dispose() => _tones.dispose();
}

/// [MemCallSession] as the bloc's [CallMedia].
class _SessionMedia implements CallMedia {
  _SessionMedia(this._s);

  final MemCallSession _s;

  @override
  set onCandidates(void Function(List<String>)? f) => _s.onCandidates = f;
  @override
  set onState(void Function(MemCallState)? f) => _s.onState = f;
  @override
  set onPath(void Function(MemCallPath)? f) => _s.onPath = f;
  @override
  Future<String> offer() => _s.offer();
  @override
  Future<String> restartOffer() => _s.restartOffer();
  @override
  Future<String> answer(String offerSdp) => _s.answer(offerSdp);
  @override
  Future<String?> answerRestart(String offerSdp, {required bool polite}) =>
      _s.answerRestart(offerSdp, polite: polite);
  @override
  Future<void> answered(String answerSdp) => _s.answered(answerSdp);
  @override
  Future<void> addCandidates(List<String> lines) => _s.addCandidates(lines);
  @override
  void mute({required bool muted}) => _s.mute(muted: muted);
  @override
  Future<void> speaker({required bool on}) => _s.speaker(on: on);
  @override
  Future<void> close() => _s.close();
}
