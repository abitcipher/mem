import 'dart:async';

import 'package:material_ui/material_ui.dart';
import 'package:mem_calls/mem_calls.dart';

import '../app_scope.dart';
import '../l10n/gen/app_localizations.dart';
import '../src/rust/api.dart';
import '../widgets/grade_badge.dart';
import 'call_controller.dart';

/// The call in progress: who, where it stands, the grade of the media path (live), and the
/// buttons for this moment. Closes itself when the call is over.
class CallScreen extends StatefulWidget {
  const CallScreen({super.key});

  @override
  State<CallScreen> createState() => _CallScreenState();
}

class _CallScreenState extends State<CallScreen> {
  Timer? _clock;
  bool _closing = false;

  @override
  void initState() {
    super.initState();
    _clock = Timer.periodic(const Duration(seconds: 1), (_) {
      if (mounted) setState(() {});
    });
  }

  @override
  void dispose() {
    _clock?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final state = AppScope.of(context);
    final calls = state.calls;
    return ListenableBuilder(
      listenable: calls,
      builder: (context, _) {
        final c = calls.call;
        if (c == null) {
          if (!_closing) {
            _closing = true;
            WidgetsBinding.instance.addPostFrameCallback((_) {
              if (mounted) Navigator.of(context).maybePop();
            });
          }
          return const Scaffold();
        }
        final contact = state.node
            .contacts()
            .where((x) => x.id == c.contactId)
            .firstOrNull;
        final name = contact == null ? '' : contactTitle(contact);
        final status = switch (c.phase) {
          CallPhase.calling => l.callCalling,
          CallPhase.ringing => l.callIncoming,
          CallPhase.connecting => l.callConnecting,
          CallPhase.talking => _elapsed(c.talkingSince),
          CallPhase.ended => _ended(l, c.endReason),
        };
        final (label, grade) = switch (c.path) {
          MemCallPath.wifiDirect => (l.channelWifiDirect, 'P0'),
          MemCallPath.localNetwork => (l.channelLan, 'P1'),
          MemCallPath.servers => (l.channelHub, 'P2'),
          null => ('', ''),
        };
        return Scaffold(
          body: SafeArea(
            child: Padding(
              padding: const EdgeInsets.all(24),
              child: Column(
                children: [
                  const Spacer(),
                  CircleAvatar(
                    radius: 48,
                    child: Text(
                      name.isEmpty ? '?' : name.characters.first.toUpperCase(),
                      style: const TextStyle(fontSize: 40),
                    ),
                  ),
                  const SizedBox(height: 16),
                  Text(
                    name,
                    style: Theme.of(context).textTheme.headlineSmall,
                    textAlign: TextAlign.center,
                  ),
                  const SizedBox(height: 8),
                  Text(status, textAlign: TextAlign.center),
                  const SizedBox(height: 12),
                  // Honest provenance, live: the path the media actually takes.
                  if (grade.isNotEmpty) GradeBadge(label: label, grade: grade),
                  const Spacer(),
                  if (c.phase == CallPhase.ringing)
                    Row(
                      mainAxisAlignment: MainAxisAlignment.spaceEvenly,
                      children: [
                        _RoundButton(
                          icon: Icons.call_end,
                          label: l.callDecline,
                          color: Colors.red,
                          onPressed: calls.decline,
                        ),
                        _RoundButton(
                          icon: Icons.call,
                          label: l.callAccept,
                          color: Colors.green,
                          onPressed: calls.accept,
                        ),
                      ],
                    )
                  else if (c.phase != CallPhase.ended)
                    Row(
                      mainAxisAlignment: MainAxisAlignment.spaceEvenly,
                      children: [
                        _RoundButton(
                          icon: c.muted ? Icons.mic_off : Icons.mic,
                          label: c.muted ? l.callUnmute : l.callMute,
                          onPressed: calls.toggleMute,
                        ),
                        _RoundButton(
                          icon: c.speaker ? Icons.volume_up : Icons.hearing,
                          label: l.callSpeaker,
                          onPressed: calls.toggleSpeaker,
                        ),
                        _RoundButton(
                          icon: Icons.call_end,
                          label: l.callHangUp,
                          color: Colors.red,
                          onPressed: calls.hangUp,
                        ),
                      ],
                    ),
                  const SizedBox(height: 24),
                ],
              ),
            ),
          ),
        );
      },
    );
  }

  String _elapsed(DateTime? since) {
    final d = since == null ? Duration.zero : DateTime.now().difference(since);
    final s = (d.inSeconds % 60).toString().padLeft(2, '0');
    return '${d.inMinutes}:$s';
  }

  String _ended(AppLocalizations l, CallEndReason? r) => switch (r) {
    CallEndReason.declined => l.callDeclined,
    CallEndReason.busy => l.callBusy,
    CallEndReason.failed => l.callFailed,
    CallEndReason.unanswered => l.callUnanswered,
    CallEndReason.hungUp || null => l.callEnded,
  };
}

class _RoundButton extends StatelessWidget {
  const _RoundButton({
    required this.icon,
    required this.label,
    required this.onPressed,
    this.color,
  });

  final IconData icon;
  final String label;
  final VoidCallback onPressed;
  final Color? color;

  @override
  Widget build(BuildContext context) {
    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        IconButton.filled(
          iconSize: 32,
          style: color == null
              ? null
              : IconButton.styleFrom(
                  backgroundColor: color,
                  foregroundColor: Colors.white,
                ),
          tooltip: label,
          onPressed: onPressed,
          icon: Icon(icon),
        ),
        const SizedBox(height: 6),
        Text(label, style: Theme.of(context).textTheme.labelMedium),
      ],
    );
  }
}
