import 'dart:async';
import 'dart:math';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:mem_radio/mem_radio.dart';

import '../app_scope.dart';
import '../l10n/gen/app_localizations.dart';
import '../src/rust/api.dart';
import '../src/ui_settings.dart';
import '../widgets/help.dart';
import 'receive.dart';

/// What the phones exchange by sound.
enum SoundMode {
  /// Contact codes, to pair.
  pair,

  /// Where to reach each other on this Wi-Fi, to connect.
  connect,
}

/// Pairing or connecting by sound: both phones play their object (a contact code, or a connect
/// hint) and listen for the other's. Messages never travel by sound. The microphone runs only
/// while this screen is open. When the other object arrives this phone keeps playing one more
/// round, so the other one can finish too.
class SoundPairScreen extends StatefulWidget {
  const SoundPairScreen({this.mode = SoundMode.pair, super.key});

  final SoundMode mode;

  @override
  State<SoundPairScreen> createState() => _SoundPairScreenState();
}

enum _Phase { starting, running, stopped, noMicrophone, failed, noNetwork }

class _SoundPairScreenState extends State<SoundPairScreen> {
  _Phase _phase = _Phase.starting;
  SoundSession? _session;
  StreamSubscription<RadioEvent>? _mic;
  Future<void> _listening = Future.value();
  bool _open = true;
  bool _got = false;
  int _received = 0;
  int _needed = 0;
  final _random = Random();

  /// Counts starts: a playing loop from an earlier start stops when this changes.
  int _run = 0;

  /// The code is played at most this many rounds, then the screen stops
  /// and offers to try again. After the other code arrived, one more round lets the other phone
  /// finish.
  static const _maxRounds = 3;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) => _start());
  }

  Future<void> _start() async {
    final state = AppScope.read(context);
    if (!await MemRadio.soundRequestPermission()) {
      if (mounted) setState(() => _phase = _Phase.noMicrophone);
      return;
    }
    // Each session holds one of ggwave's four receivers: free the previous one now.
    _session?.dispose();
    _session = null;
    final profile = state.settings.soundTone == SoundTone.inaudible
        ? SoundProfile.ultrasound
        : SoundProfile.audible;
    try {
      _session = widget.mode == SoundMode.connect
          ? await state.node.startSoundConnect(profile: profile)
          : await state.node.startSoundPairing(profile: profile);
    } on Object catch (e) {
      final phase = e == 'no_local_network' ? _Phase.noNetwork : _Phase.failed;
      if (mounted) setState(() => _phase = phase);
      return;
    }
    _mic = MemRadio.events.listen((e) {
      if (e is SoundIn) {
        // One at a time, in order: each chunk continues the previous one.
        _listening = _listening.then((_) => _hear(e.data));
      }
    });
    if (!await MemRadio.soundStart()) {
      if (mounted) setState(() => _phase = _Phase.noMicrophone);
      return;
    }
    if (!mounted) return;
    setState(() => _phase = _Phase.running);
    unawaited(_play());
  }

  /// Stops playing and the microphone.
  Future<void> _halt() async {
    await _mic?.cancel();
    _mic = null;
    await MemRadio.soundStop();
  }

  /// Starts over with a new code session: after "Try again", or when the tone changes while
  /// the code is playing, so the new tone is heard at once.
  Future<void> _restart() async {
    _run++;
    await _halt();
    if (!mounted) return;
    setState(() {
      _phase = _Phase.starting;
      _received = 0;
      _needed = 0;
    });
    await _start();
  }

  /// Plays our code frame by frame. A phone cannot hear past its own loudspeaker, so before each
  /// frame it waits a random moment and then for as long as the other phone is heard playing.
  Future<void> _play() async {
    final session = _session;
    if (session == null) return;
    final run = _run;
    bool current() => _open && run == _run;
    final round = session.framesPerRound();
    var bursts = 0;
    var gotAt = -1;
    while (current()) {
      if (_got && gotAt < 0) gotAt = bursts;
      final done = _got
          ? bursts - gotAt >= round
          : bursts >= _maxRounds * round;
      if (done) break;
      await Future<void>.delayed(
        Duration(milliseconds: 800 + _random.nextInt(2200)),
      );
      while (current() && session.busy()) {
        await Future<void>.delayed(const Duration(milliseconds: 300));
      }
      if (!current()) break;
      try {
        final pcm = await session.nextBurst();
        if (!current()) break;
        await MemRadio.soundPlay(
          pcm.buffer.asUint8List(pcm.offsetInBytes, pcm.lengthInBytes),
        );
        bursts++;
      } on Object {
        if (mounted && current()) setState(() => _phase = _Phase.failed);
        return;
      }
    }
    if (!current()) return;
    // Give a transmission that started during our last burst time to arrive, then stop.
    await Future<void>.delayed(const Duration(seconds: 8));
    if (!current()) return;
    await _halt();
    if (mounted && !_got) setState(() => _phase = _Phase.stopped);
  }

  Future<void> _hear(Uint8List bytes) async {
    final session = _session;
    if (session == null || !_open || _got) return;
    // Little-endian 16-bit samples; copy, since the bytes may not be 2-aligned.
    final data = ByteData.sublistView(bytes);
    final pcm = Int16List(bytes.length ~/ 2);
    for (var i = 0; i < pcm.length; i++) {
      pcm[i] = data.getInt16(i * 2, Endian.little);
    }
    final state = AppScope.read(context);
    final ScanUpdate update;
    try {
      update = await state.node.soundListen(session: session, pcm: pcm);
    } on Object {
      return;
    }
    if (!mounted) return;
    switch (update) {
      case ScanUpdate_Progress(:final received, :final needed):
        setState(() {
          _received = received;
          _needed = needed;
        });
      case ScanUpdate_Received(:final field0):
        _got = true;
        setState(() => _received = _needed);
        state.changed();
        await reportOutcome(context, field0);
      case ScanUpdate_Connecting():
        _got = true;
        setState(() => _received = _needed);
        state.changed();
      case ScanUpdate_Nothing() ||
          ScanUpdate_Backup() ||
          ScanUpdate_DeviceSetup() ||
          ScanUpdate_DeviceRequest() ||
          ScanUpdate_Attest():
    }
  }

  @override
  void dispose() {
    _open = false;
    unawaited(_mic?.cancel());
    unawaited(MemRadio.soundStop());
    _session?.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final state = AppScope.of(context);
    final tone = state.settings.soundTone;
    final connect = widget.mode == SoundMode.connect;
    final title = connect ? l.connectBySound : l.soundPairTitle;
    return Scaffold(
      appBar: AppBar(
        title: Text(title),
        actions: [
          HelpAction(
            title: title,
            text: connect
                ? '${l.connectSoundHint}\n\n${l.connectSoundExposure}'
                : '${l.soundPairHint}\n\n${l.soundPairExposure}',
          ),
        ],
      ),
      body: ListView(
        padding: const EdgeInsets.all(24),
        children: [
          Center(
            child: switch (_phase) {
              _Phase.starting => const CircularProgressIndicator(),
              _Phase.running => Column(
                children: [
                  Icon(
                    _got ? Icons.check_circle_outline : Icons.graphic_eq,
                    size: 64,
                  ),
                  const SizedBox(height: 12),
                  Text(
                    _got
                        ? (connect ? l.connectGot : l.soundPairGot)
                        : _needed > 0
                        ? l.soundPairProgress(_received, _needed)
                        : l.soundPairListening,
                  ),
                ],
              ),
              _Phase.stopped => Column(
                children: [
                  Text(l.soundPairStopped, textAlign: TextAlign.center),
                  const SizedBox(height: 12),
                  FilledButton.icon(
                    icon: const Icon(Icons.replay),
                    label: Text(l.soundPairTryAgain),
                    onPressed: _restart,
                  ),
                ],
              ),
              _Phase.noMicrophone => Text(
                connect ? l.connectNoMicrophone : l.soundPairNoMicrophone,
                textAlign: TextAlign.center,
              ),
              _Phase.failed => Text(
                connect ? l.connectFailed : l.soundPairFailed,
                textAlign: TextAlign.center,
              ),
              _Phase.noNetwork => Text(
                l.reason_no_local_network,
                textAlign: TextAlign.center,
              ),
            },
          ),
          const SizedBox(height: 24),
          Text(l.soundToneTitle),
          const SizedBox(height: 8),
          SegmentedButton<SoundTone>(
            showSelectedIcon: false,
            segments: [
              ButtonSegment(
                value: SoundTone.audible,
                label: Text(l.soundToneAudible),
              ),
              ButtonSegment(
                value: SoundTone.inaudible,
                label: Text(l.soundToneInaudible),
              ),
            ],
            selected: {tone},
            // While the code is playing, it starts over in the new tone. Either tone is
            // heard, so the two phones need not match.
            onSelectionChanged: (v) async {
              state.updateSettings((s) => s.soundTone = v.first);
              if (_phase == _Phase.running && !_got) await _restart();
            },
          ),
        ],
      ),
    );
  }
}
