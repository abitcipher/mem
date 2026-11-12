import 'dart:async';
import 'dart:math';
import 'dart:typed_data';

import 'package:material_ui/material_ui.dart';
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

/// Pairing or connecting by sound. The user starts it: one phone listens, the other plays its
/// object (a contact code, or a connect hint). A phone never plays and listens at once, so the two
/// never talk over each other. Messages never travel by sound; the microphone runs only while
/// this phone is listening.
///
/// Pairing then goes on by itself: the listener, once it has the other's code, waits for quiet
/// and plays its own; the player, after each round, listens for the answer. When a phone has the
/// other's code and has played its own, it asks its user to compare the symbols and connect or
/// decline, so both phones ask at the end of the exchange.
class SoundPairScreen extends StatefulWidget {
  const SoundPairScreen({this.mode = SoundMode.pair, super.key});

  final SoundMode mode;

  @override
  State<SoundPairScreen> createState() => _SoundPairScreenState();
}

enum _Role { idle, listening, waiting, playing }

enum _Note {
  none,
  listenTimeout,
  playDone,
  noAnswer,
  noMicrophone,
  failed,
  noNetwork,
  paired,
  declined,
  declinedKept,
}

class _SoundPairScreenState extends State<SoundPairScreen> {
  _Role _role = _Role.idle;
  _Note _note = _Note.none;
  SoundSession? _session;
  StreamSubscription<RadioEvent>? _mic;
  Future<void> _listening = Future.value();
  Timer? _listenLimit;
  bool _open = true;
  final _random = Random();

  /// The other phone's object arrived (in this exchange).
  bool _got = false;

  /// We played at least one full round of our code (in this exchange).
  bool _played = false;

  /// The contact the other phone's code added or confirmed, waiting for this user's answer.
  ReceiveOutcome_Contact? _pending;
  bool _asked = false;

  int _received = 0;
  int _needed = 0;
  int _round = 0;

  /// Counts exchanges: the loops of an earlier one stop when this changes.
  int _run = 0;

  bool get _pairing => widget.mode == SoundMode.pair;

  /// Our code is played at most this many rounds.
  static const _rounds = 3;

  /// After a round the player listens this long for the other phone to start answering.
  static const _answerWithin = Duration(seconds: 8);

  /// Listening stops by itself after this long without a complete code.
  static const _listenFor = Duration(minutes: 2);

  /// Opens the sound session on first use, or again after the tone changed.
  Future<SoundSession?> _ensureSession() async {
    final existing = _session;
    if (existing != null) return existing;
    final state = AppScope.read(context);
    final profile = state.settings.soundTone == SoundTone.inaudible
        ? SoundProfile.ultrasound
        : SoundProfile.audible;
    try {
      final session = _pairing
          ? await state.node.startSoundPairing(profile: profile)
          : await state.node.startSoundConnect(profile: profile);
      if (!mounted) {
        session.dispose();
        return null;
      }
      return _session = session;
    } on Object catch (e) {
      if (mounted) {
        setState(() {
          _role = _Role.idle;
          _note = e == 'no_local_network' ? _Note.noNetwork : _Note.failed;
        });
      }
      return null;
    }
  }

  /// Turns the microphone on; false if it cannot start.
  Future<bool> _micOn(SoundSession session, int run) async {
    if (_mic != null) return true;
    _mic = MemRadio.events.listen((e) {
      if (e is SoundIn) {
        // One at a time, in order: each chunk continues the previous one.
        _listening = _listening.then((_) => _hear(session, run, e.data));
      }
    });
    if (await MemRadio.soundStart()) return true;
    await _micOff();
    return false;
  }

  Future<void> _micOff() async {
    _listenLimit?.cancel();
    _listenLimit = null;
    await _mic?.cancel();
    _mic = null;
    await MemRadio.soundStop();
  }

  /// Ends the exchange: stops playing and listening.
  Future<void> _stop({_Note note = _Note.none}) async {
    _run++;
    await _micOff();
    if (mounted) {
      setState(() {
        _role = _Role.idle;
        _note = note;
      });
    }
  }

  /// A new exchange: forgets what the previous one got and played.
  int _begin() {
    _run++;
    _got = false;
    _played = false;
    _pending = null;
    _asked = false;
    _received = 0;
    _needed = 0;
    _note = _Note.none;
    return _run;
  }

  /// The user's "Listen".
  Future<void> _listen() async {
    await _micOff();
    final run = _begin();
    if (!await MemRadio.soundRequestPermission()) {
      if (mounted) setState(() => _note = _Note.noMicrophone);
      return;
    }
    final session = await _ensureSession();
    if (session == null || !mounted || run != _run) return;
    if (!await _micOn(session, run)) {
      await _stop(note: _Note.noMicrophone);
      return;
    }
    if (!mounted || run != _run) return;
    _limitListening(run);
    setState(() => _role = _Role.listening);
  }

  void _limitListening(int run) {
    _listenLimit?.cancel();
    _listenLimit = Timer(_listenFor, () {
      if (_role == _Role.listening && run == _run && !_got) {
        unawaited(_stop(note: _Note.listenTimeout));
      }
    });
  }

  /// The user's "Play my code".
  Future<void> _start() async {
    await _micOff();
    final run = _begin();
    if (_pairing && !await MemRadio.soundRequestPermission()) {
      // Pairing needs to hear the answer.
      if (mounted) setState(() => _note = _Note.noMicrophone);
      return;
    }
    final session = await _ensureSession();
    if (session == null || !mounted || run != _run) return;
    await _broadcast(session, run);
  }

  /// Plays our code round by round. While pairing and still without the other's code, it
  /// listens after each round for the answer; once the other phone is heard it stays listening.
  Future<void> _broadcast(SoundSession session, int run) async {
    bool current() => _open && run == _run;
    final perRound = session.framesPerRound();
    for (var round = 1; round <= _rounds; round++) {
      if (!current()) return;
      // The user answered the prompt: the other phone has our code, no more rounds needed.
      if (_asked && _pending == null) break;
      setState(() {
        _role = _Role.playing;
        _round = round;
      });
      for (var i = 0; i < perRound; i++) {
        if (!current()) return;
        try {
          final pcm = await session.nextBurst();
          if (!current()) return;
          await MemRadio.soundPlay(
            pcm.buffer.asUint8List(pcm.offsetInBytes, pcm.lengthInBytes),
          );
        } on Object {
          if (current()) await _stop(note: _Note.failed);
          return;
        }
        // A short gap between frames lets the listener tell them apart.
        await Future<void>.delayed(const Duration(milliseconds: 300));
      }
      _played = true;
      if (!current()) return;
      if (_got) {
        // Our turn after theirs: the other phone has heard a whole round; ask now and play
        // the remaining rounds in case it missed a frame.
        unawaited(_ask());
        continue;
      }
      if (!_pairing) continue;
      // Listen for the answer.
      if (!await _micOn(session, run)) {
        await _stop(note: _Note.noMicrophone);
        return;
      }
      if (!current()) return;
      setState(() {
        _role = _Role.listening;
        _received = 0;
        _needed = 0;
      });
      final until = DateTime.now().add(_answerWithin);
      while (current() && DateTime.now().isBefore(until)) {
        if (_received > 0 || session.busy()) {
          // The other phone answers: stay listening until its code is complete.
          _limitListening(run);
          return;
        }
        await Future<void>.delayed(const Duration(milliseconds: 250));
      }
      if (!current() || _got) return;
      await _micOff();
    }
    if (!current()) return;
    if (_got) {
      setState(() => _role = _Role.idle);
    } else {
      await _stop(note: _pairing ? _Note.noAnswer : _Note.playDone);
    }
  }

  /// Our turn after the other phone's code arrived: wait until it is quiet, then play.
  Future<void> _takeTurn(SoundSession session, int run) async {
    setState(() => _role = _Role.waiting);
    while (_open && run == _run && session.busy()) {
      await Future<void>.delayed(const Duration(milliseconds: 250));
    }
    await Future<void>.delayed(
      Duration(milliseconds: 300 + _random.nextInt(700)),
    );
    if (!_open || run != _run) return;
    await _broadcast(session, run);
  }

  Future<void> _hear(SoundSession session, int run, Uint8List bytes) async {
    if (!_open || run != _run || _got || session != _session) return;
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
    if (!mounted || run != _run || _got) return;
    switch (update) {
      case ScanUpdate_Progress(:final received, :final needed):
        setState(() {
          _received = received;
          _needed = needed;
        });
      case ScanUpdate_Received(:final field0):
        _got = true;
        await _micOff();
        if (!mounted) return;
        setState(() => _received = _needed);
        state.changed();
        if (field0 is! ReceiveOutcome_Contact) {
          // Not a contact code (rejected, not MEM): say so and end.
          await _stop();
          if (mounted) await reportOutcome(context, field0);
          return;
        }
        _pending = field0;
        if (_played) {
          // We played first: the exchange is complete.
          setState(() => _role = _Role.idle);
          await _ask();
        } else {
          await _takeTurn(session, run);
        }
      case ScanUpdate_Connecting():
        _got = true;
        await _micOff();
        if (!mounted) return;
        setState(() {
          _received = _needed;
          _role = _Role.idle;
        });
        state.changed();
      case ScanUpdate_Nothing() ||
          ScanUpdate_Backup() ||
          ScanUpdate_DeviceSetup() ||
          ScanUpdate_DeviceRequest() ||
          ScanUpdate_Attest():
    }
  }

  /// Asks this user to compare the symbols and connect or decline. A declined code that added
  /// a new contact removes it again; a contact known before stays as it was.
  Future<void> _ask() async {
    final pending = _pending;
    if (pending == null || _asked || !mounted) return;
    _asked = true;
    final l = AppLocalizations.of(context);
    final connect = await showDialog<bool>(
      context: context,
      barrierDismissible: false,
      builder: (context) => AlertDialog(
        title: Text(
          pending.updated ? l.contactUpdatedTitle : l.contactAddedTitle,
        ),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(l.compareSas),
            const SizedBox(height: 20),
            Text(
              pending.sas,
              style: const TextStyle(fontSize: 34),
              textAlign: TextAlign.center,
            ),
          ],
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context, false),
            child: Text(l.soundDecline),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(context, true),
            child: Text(l.soundConnect),
          ),
        ],
      ),
    );
    if (!mounted) return;
    final state = AppScope.read(context);
    if (connect != true && !pending.updated) {
      try {
        await state.node.removeContact(contactId: pending.contactId);
      } on Object {
        // Already gone.
      }
      state.changed();
    }
    if (!mounted) return;
    setState(() {
      _pending = null;
      _note = connect == true
          ? _Note.paired
          : pending.updated
          ? _Note.declinedKept
          : _Note.declined;
    });
  }

  @override
  void dispose() {
    _open = false;
    _run++;
    _listenLimit?.cancel();
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

    final String status = switch (_role) {
      _Role.listening =>
        _needed > 0
            ? l.soundPairProgress(_received, _needed)
            : l.soundListening,
      _Role.waiting => l.soundTurnWait,
      _Role.playing => l.soundPlaying(_round, _rounds),
      _Role.idle => switch (_note) {
        _Note.listenTimeout => l.soundListenTimeout,
        _Note.playDone => l.soundPlayDone,
        _Note.noAnswer => l.soundNoAnswer,
        _Note.noMicrophone =>
          connect ? l.connectNoMicrophone : l.soundPairNoMicrophone,
        _Note.failed => connect ? l.connectFailed : l.soundPairFailed,
        _Note.noNetwork => l.reason_no_local_network,
        _Note.paired => l.soundPaired,
        _Note.declined => l.soundDeclined,
        _Note.declinedKept => l.soundDeclinedKept,
        _Note.none =>
          _got ? (connect ? l.connectGot : l.soundPairGot) : l.soundIdle,
      },
    };
    final running = _role != _Role.idle;

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
          Text(connect ? l.connectSoundHint : l.soundPairHint),
          const SizedBox(height: 24),
          // The user only starts the exchange (and may stop it); the turns that follow are
          // automatic.
          if (running)
            SizedBox(
              height: 56,
              child: FilledButton.icon(
                icon: const Icon(Icons.stop),
                label: Text(l.soundStop),
                onPressed: _stop,
              ),
            )
          else
            Row(
              children: [
                Expanded(
                  child: SizedBox(
                    height: 56,
                    child: FilledButton.tonalIcon(
                      icon: const Icon(Icons.hearing),
                      label: Text(l.soundListen),
                      onPressed: _listen,
                    ),
                  ),
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: SizedBox(
                    height: 56,
                    child: FilledButton.tonalIcon(
                      icon: const Icon(Icons.campaign_outlined),
                      label: Text(l.soundPlay),
                      onPressed: _start,
                    ),
                  ),
                ),
              ],
            ),
          const SizedBox(height: 24),
          Center(
            child: Icon(switch (_role) {
              _Role.listening => Icons.hearing,
              _Role.playing => Icons.graphic_eq,
              _Role.waiting => Icons.hourglass_top,
              _Role.idle =>
                _got ? Icons.check_circle_outline : Icons.volume_up_outlined,
            }, size: 56),
          ),
          const SizedBox(height: 12),
          Text(status, textAlign: TextAlign.center),
          if (_role == _Role.listening && _needed > 0) ...[
            const SizedBox(height: 12),
            LinearProgressIndicator(value: _received / _needed),
          ],
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
            // A new tone needs a new session: a running exchange stops, to be started again.
            onSelectionChanged: (v) async {
              state.updateSettings((s) => s.soundTone = v.first);
              await _stop();
              _session?.dispose();
              _session = null;
            },
          ),
        ],
      ),
    );
  }
}
