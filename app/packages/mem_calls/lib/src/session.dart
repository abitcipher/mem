import 'dart:async';

import 'package:flutter_webrtc/flutter_webrtc.dart';

import 'path.dart';

/// Where a call stands.
enum MemCallState {
  /// Setting up the media.
  connecting,

  /// Talking.
  connected,

  /// The media could not connect, or the connection was lost.
  failed,

  /// Ended.
  closed,
}

/// One voice call's media. Audio only. Candidates and descriptions travel through the
/// callbacks and methods; this class never contacts any server unless [iceServers] names one.
class MemCallSession {
  MemCallSession._(this._pc, this._mic);

  final RTCPeerConnection _pc;
  final MediaStream _mic;
  final _pendingRemote = <RTCIceCandidate>[];
  final _outgoing = <String>[];
  Timer? _flush;
  Timer? _stats;
  bool _remoteSet = false;
  bool _closed = false;

  /// Called with batches of our ICE candidates (candidate lines), to send to the other side.
  void Function(List<String> candidates)? onCandidates;

  /// Called when the call's state changes.
  void Function(MemCallState state)? onState;

  /// Called when the media path is known or changes.
  void Function(MemCallPath path)? onPath;

  /// Opens the microphone and prepares a call. [iceServers] are STUN/TURN servers in WebRTC's
  /// form; empty means direct paths only (local network, Wi-Fi Direct).
  static Future<MemCallSession> open({
    List<Map<String, dynamic>> iceServers = const [],
  }) async {
    final mic = await navigator.mediaDevices.getUserMedia({
      'audio': {
        'echoCancellation': true,
        'noiseSuppression': true,
        'autoGainControl': true,
      },
      'video': false,
    });
    final pc = await createPeerConnection({
      'iceServers': iceServers,
      'sdpSemantics': 'unified-plan',
      // Gather once and keep; no candidate pool on mobile networks.
      'iceCandidatePoolSize': 0,
    });
    for (final track in mic.getAudioTracks()) {
      await pc.addTrack(track, mic);
    }
    final s = MemCallSession._(pc, mic);
    pc.onIceCandidate = s._localCandidate;
    pc.onConnectionState = s._connectionState;
    return s;
  }

  /// The caller's side: our offer's description, to send.
  Future<String> offer() async {
    final d = await _pc.createOffer({'offerToReceiveAudio': true});
    await _pc.setLocalDescription(d);
    return d.sdp ?? '';
  }

  /// The callee's side: takes the caller's [offerSdp] and returns our answer, to send.
  Future<String> answer(String offerSdp) async {
    await _setRemote(RTCSessionDescription(offerSdp, 'offer'));
    final d = await _pc.createAnswer({'offerToReceiveAudio': true});
    await _pc.setLocalDescription(d);
    return d.sdp ?? '';
  }

  /// The caller's side: the callee's answer arrived.
  Future<void> answered(String answerSdp) =>
      _setRemote(RTCSessionDescription(answerSdp, 'answer'));

  /// Candidates from the other side. One audio stream: they all belong to media line 0.
  Future<void> addCandidates(List<String> lines) async {
    for (final line in lines) {
      final c = RTCIceCandidate(line, '0', 0);
      if (_remoteSet) {
        await _pc.addCandidate(c);
      } else {
        _pendingRemote.add(c);
      }
    }
  }

  /// Mutes or unmutes our microphone.
  void mute({required bool muted}) {
    for (final t in _mic.getAudioTracks()) {
      t.enabled = !muted;
    }
  }

  /// Plays through the loudspeaker instead of the earpiece.
  Future<void> speaker({required bool on}) => Helper.setSpeakerphoneOn(on);

  /// Ends the media and releases the microphone. Safe to call twice.
  Future<void> close() async {
    if (_closed) return;
    _closed = true;
    _flush?.cancel();
    _stats?.cancel();
    for (final t in _mic.getTracks()) {
      await t.stop();
    }
    await _mic.dispose();
    await _pc.close();
    onState?.call(MemCallState.closed);
  }

  Future<void> _setRemote(RTCSessionDescription d) async {
    await _pc.setRemoteDescription(d);
    _remoteSet = true;
    for (final c in _pendingRemote) {
      await _pc.addCandidate(c);
    }
    _pendingRemote.clear();
  }

  /// Batches our candidates briefly, so a call sends a few messages rather than one per
  /// candidate.
  void _localCandidate(RTCIceCandidate c) {
    final line = c.candidate;
    if (line == null || line.isEmpty) return;
    _outgoing.add(line);
    _flush ??= Timer(const Duration(milliseconds: 300), () {
      _flush = null;
      if (_outgoing.isEmpty) return;
      final batch = List<String>.of(_outgoing);
      _outgoing.clear();
      onCandidates?.call(batch);
    });
  }

  void _connectionState(RTCPeerConnectionState state) {
    final mapped = switch (state) {
      RTCPeerConnectionState.RTCPeerConnectionStateConnected =>
        MemCallState.connected,
      RTCPeerConnectionState.RTCPeerConnectionStateFailed ||
      RTCPeerConnectionState.RTCPeerConnectionStateDisconnected =>
        MemCallState.failed,
      RTCPeerConnectionState.RTCPeerConnectionStateClosed =>
        MemCallState.closed,
      _ => MemCallState.connecting,
    };
    if (mapped == MemCallState.connected) {
      unawaited(_readPath());
      _stats ??= Timer.periodic(const Duration(seconds: 5), (_) => _readPath());
    }
    if (!_closed) onState?.call(mapped);
  }

  /// Reads the path of the selected candidate pair from the connection's statistics.
  Future<void> _readPath() async {
    if (_closed) return;
    final reports = await _pc.getStats();
    final byId = {for (final r in reports) r.id: r};
    for (final r in reports) {
      final v = r.values;
      final selected =
          r.type == 'candidate-pair' &&
          v['state'] == 'succeeded' &&
          (v['nominated'] ?? true) == true;
      if (!selected) continue;
      final local = byId[v['localCandidateId']]?.values;
      final path = pathOf(
        local?['candidateType'] as String?,
        (local?['address'] ?? local?['ip']) as String?,
      );
      if (path != null) onPath?.call(path);
      return;
    }
  }
}
