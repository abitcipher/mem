import 'package:flutter_test/flutter_test.dart';
import 'package:mem_calls/mem_calls.dart';

void main() {
  test('the media path follows the selected local candidate', () {
    expect(pathOf('host', '192.168.49.1'), MemCallPath.wifiDirect);
    expect(pathOf('host', '10.0.0.5'), MemCallPath.localNetwork);
    expect(pathOf('relay', '144.31.216.27'), MemCallPath.servers);
    expect(pathOf('srflx', null), MemCallPath.servers);
    expect(pathOf('unknown', '10.0.0.5'), isNull);
  });

  test('tones are split into incoming and outgoing, by stable id', () {
    expect(MemTone.forIncoming, isNot(contains(MemTone.ringback)));
    expect(MemTone.forOutgoing, [MemTone.ringback, MemTone.beep]);
    expect(MemTone.byId('chime'), MemTone.chime);
    expect(MemTone.byId('off'), isNull);
  });
}
