import 'package:flutter_test/flutter_test.dart';

/// The blend rule is the one piece of harmonic mixing the player performs, and
/// it is pure arithmetic — so it is worth a check that does not need audio.
int blend(int max, String? a, String? b) {
  if (max <= 0) return 0;
  if (a == null || b == null) return (max * 2 / 3).round().clamp(4, max);
  if (a == b) return max;
  final an = int.tryParse(a.substring(0, a.length - 1));
  final bn = int.tryParse(b.substring(0, b.length - 1));
  final al = a.substring(a.length - 1);
  final bl = b.substring(b.length - 1);
  if (an != null && bn != null) {
    if (an == bn && al != bl) return (max * 2 / 3).round().clamp(4, max);
    final d = (an - bn).abs();
    if (al == bl && (d == 1 || d == 11)) {
      return (max * 2 / 3).round().clamp(4, max);
    }
  }
  return (max / 3).round().clamp(2, max);
}

void main() {
  test('the same key blends for the whole length', () {
    expect(blend(12, '8A', '8A'), 12);
  });
  test('a neighbour blends shorter', () {
    expect(blend(12, '8A', '9A'), 8);
    expect(blend(12, '12A', '1A'), 8);
    expect(blend(12, '8A', '8B'), 8);
  });
  test('a clash gets a short blend', () {
    expect(blend(12, '8A', '3A'), 4);
  });
  test('an unknown key is modest, not silent', () {
    expect(blend(12, null, '3A'), 8);
    expect(blend(0, '8A', '8A'), 0);
  });
}
