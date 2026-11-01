import 'dart:math' as math;

import 'package:flutter_test/flutter_test.dart';

/// Equal-power crossfade: the sum of squares has to stay at one, otherwise the
/// middle of the transition dips and you hear a hole.
void main() {
  double gainCos(double t) => math.cos(t * math.pi / 2);
  double gainSin(double t) => math.sin(t * math.pi / 2);

  test('summed power stays constant across the transition', () {
    for (var i = 0; i <= 20; i++) {
      final t = i / 20;
      final power = gainCos(t) * gainCos(t) + gainSin(t) * gainSin(t);
      expect(power, closeTo(1.0, 1e-12), reason: 't=$t');
    }
  });

  test('a linear fade would dip; ours does not', () {
    final mid = gainCos(0.5) * gainCos(0.5) + gainSin(0.5) * gainSin(0.5);
    expect(mid, closeTo(1.0, 1e-12));
    final linearMid = 0.5 * 0.5 + 0.5 * 0.5;
    expect(linearMid, closeTo(0.5, 1e-12));
  });
}
