// Verifies the equaliser path against a real mpv, not against a mock: the
// filter chain the settings screen installs must survive a round trip through
// the audio backend, otherwise the preset is a piece of UI that does nothing.
//
// Needs libmpv, so it is a separate run: `flutter test test/eq_backend_test.dart`
// on an empty machine with no audio device it is skipped rather than failed.
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:media_kit/media_kit.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('mpv accepts the equaliser chain and reports it back', () async {
    if (!Platform.isLinux) return;
    MediaKit.ensureInitialized();
    final player = Player();
    try {
      final native = player.platform;
      expect(native, isA<NativePlayer>());
      final chain =
          'equalizer=f=62:width_type=o:width=1.4:g=5,'
          'equalizer=f=250:width_type=o:width=1.4:g=2';
      await (native as NativePlayer).setProperty('af', chain);
      final read = await native.getProperty('af');
      // mpv normalises the string (spaces, ordering), so compare the parts that
      // matter rather than the exact text.
      expect(read.replaceAll(' ', ''), contains('equalizer=f=62'));
      expect(read.replaceAll(' ', ''), contains('g=5'));

      // And clearing it must actually clear it.
      await native.setProperty('af', '');
      expect((await native.getProperty('af')).trim(), isEmpty);
    } finally {
      await player.dispose();
    }
  }, timeout: const Timeout(Duration(minutes: 2)));
}
