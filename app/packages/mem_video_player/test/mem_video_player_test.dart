import 'package:flutter_test/flutter_test.dart';
import 'package:mem_video_player/mem_video_player.dart';

void main() {
  test('common phone formats play; others open elsewhere', () {
    expect(MemMediaKind.of('video/mp4'), MemMediaKind.video);
    expect(MemMediaKind.of('Audio/MPEG'), MemMediaKind.audio);
    expect(MemMediaKind.of('audio/ogg; codecs=opus'), MemMediaKind.audio);
    expect(MemMediaKind.of('image/jpeg'), isNull);
    expect(MemMediaKind.of('video/x-matroska'), isNull);
  });

  test('positions read as on a clock', () {
    expect(formatPosition(const Duration(seconds: 5)), '0:05');
    expect(formatPosition(const Duration(minutes: 3, seconds: 7)), '3:07');
    expect(
      formatPosition(const Duration(hours: 1, minutes: 2, seconds: 3)),
      '1:02:03',
    );
  });
}
