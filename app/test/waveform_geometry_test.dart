import 'package:bulba_music/widgets/waveform.dart';
import 'package:flutter_test/flutter_test.dart';

/// The bars have to span exactly the box, otherwise the visible half of the
/// waveform stops matching the timeline and every seek looks shifted.
void main() {
  test('bars fill the full width for whatever peak count arrives', () {
    for (final (n, width) in [
      (100, 250.0),
      (400, 250.0),
      (64, 900.0),
      (1, 300.0),
    ]) {
      final g = WaveBarGeometry.of(n, width);
      final total = n * (g.barW + g.gap);
      expect(
        total,
        closeTo(width, width * 0.001),
        reason: '$n peaks in $width px',
      );
      expect(g.barW, greaterThan(0));
    }
  });
}
