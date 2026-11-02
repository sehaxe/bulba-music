import 'dart:typed_data';

import 'package:bulba_music/cast/cast.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('the envelope round-trips', () {
    final bytes = encodeCastMessage({
      'source': 'sender-0',
      'destination': 'receiver-0',
      'namespace': 'urn:x-cast:com.google.cast.tp.connection',
      'payload': {'type': 'CONNECT'},
    });
    final len = ByteData.sublistView(bytes, 0, 4).getUint32(0);
    expect(len, bytes.length - 4, reason: 'the length prefix must match');
    final back = decodeCastMessage(bytes.sublist(4))!;
    expect(back['namespace'], 'urn:x-cast:com.google.cast.tp.connection');
    expect((back['payload'] as Map)['type'], 'CONNECT');
  });

  test('a long payload is still framed correctly', () {
    final bytes = encodeCastMessage({
      'source': 's',
      'destination': 'd',
      'namespace': 'n',
      'payload': {'type': 'LOAD', 'pad': 'x' * 300},
    });
    final len = ByteData.sublistView(bytes, 0, 4).getUint32(0);
    expect(len, bytes.length - 4);
    final back = decodeCastMessage(bytes.sublist(4))!;
    expect((back['payload'] as Map)['pad'], 'x' * 300);
  });
}
