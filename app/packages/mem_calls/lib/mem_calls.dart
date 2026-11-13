/// Voice calls on WebRTC: microphone, Opus, echo cancellation and DTLS-SRTP media.
///
/// The package does not know how call setup travels. A [MemCallSession] hands its own session
/// description and ICE candidates to callbacks, and takes the other side's through methods;
/// the caller carries them (MEM carries them end to end, which also authenticates the media's
/// DTLS fingerprints).
library;

export 'src/path.dart';
export 'src/session.dart';
