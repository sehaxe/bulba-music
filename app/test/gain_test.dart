import 'package:bulba_music/player/gain.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('a quiet master is lifted toward the target', () {
    // -20 LUFS needs +6 dB, which is exactly the ceiling.
    expect(gainForLoudness(-20), closeTo(6, 0.001));
  });

  test('a loud master is pulled down, bounded at -12', () {
    expect(gainForLoudness(-5), closeTo(-9, 0.001));
    expect(gainForLoudness(0), closeTo(-12, 0.001));
  });

  test('a track already at the target is untouched', () {
    expect(gainForLoudness(-14), closeTo(0, 0.001));
  });

  test('a track with no measurement is untouched', () {
    expect(gainForLoudness(null), 0);
  });

  test('the multiplier matches the decibels', () {
    expect(gainFactor(0), closeTo(1, 1e-9));
    expect(gainFactor(6), closeTo(1.9953, 1e-4));
    expect(gainFactor(-6), closeTo(0.5012, 1e-4));
  });
}
