import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:mem/src/ui_settings.dart';

void main() {
  test('call times stay within their hard bounds', () {
    expect(CallTimes.reach.clamp(null), 20);
    expect(CallTimes.reach.clamp(1), 10);
    expect(CallTimes.ring.clamp(100000), 120);
    expect(CallTimes.reconnect.clamp(-5), 15);
    expect(CallTimes.reconnect.clamp(90), 90);
  });

  test('a damaged settings file cannot set a call time past its bounds', () {
    final dir = Directory.systemTemp.createTempSync('mem-ui-');
    final f = File('${dir.path}/ui_settings')
      ..writeAsStringSync(
        'call_reach_s=999\ncall_ring_s=-1\ncall_reconnect_s=abc\n',
      );
    final s = UiSettings.read(f);
    expect(s.callReachSeconds, 60);
    expect(s.callRingSeconds, 15);
    expect(s.callReconnectSeconds, 60);
    dir.deleteSync(recursive: true);
  });
}
