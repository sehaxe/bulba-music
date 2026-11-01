import 'package:bulba_music/api/client.dart';
import 'package:bulba_music/widgets/waveform.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

/// The waveform is the only seek surface in the app, so its mapping from a
/// pointer to a fraction has to be exact — an off-by-a-few-percent here is what
/// made the player jump past the cursor.
void main() {
  const track = Track(
    id: 1,
    title: 'Test',
    artist: 'A',
    album: 'B',
    albumId: 1,
    hasCover: false,
  );

  Future<List<double>> pumpWave(WidgetTester tester) async {
    final seeks = <double>[];
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: Center(
            child: SizedBox(
              width: 300,
              child: TrackWaveform(
                api: Api('http://127.0.0.1:1'),
                track: track,
                progress: 0.25,
                playing: false,
                onSeek: seeks.add,
                accent: const Color(0xFFF26B3A),
                height: 48,
              ),
            ),
          ),
        ),
      ),
    );
    await tester.pump(const Duration(milliseconds: 600));
    return seeks;
  }

  testWidgets('a tap in the middle seeks to half the track', (tester) async {
    final seeks = await pumpWave(tester);
    // The waveform renders nothing without peaks, and peaks come from the
    // network. The gesture area still exists, so this checks the box mapping.
    final box = tester.getRect(find.byType(TrackWaveform));
    await tester.tapAt(Offset(box.center.dx, box.center.dy));
    await tester.pump();
    expect(seeks, isNotEmpty, reason: 'tap must reach the seek handler');
    expect(seeks.last, closeTo(0.5, 0.02));
  });

  testWidgets('a tap near the right edge seeks to the end', (tester) async {
    final seeks = await pumpWave(tester);
    final box = tester.getRect(find.byType(TrackWaveform));
    await tester.tapAt(Offset(box.right - 2, box.center.dy));
    await tester.pump();
    expect(seeks.last, greaterThan(0.98));
  });

  testWidgets('a tap near the left edge seeks to the start', (tester) async {
    final seeks = await pumpWave(tester);
    final box = tester.getRect(find.byType(TrackWaveform));
    await tester.tapAt(Offset(box.left + 2, box.center.dy));
    await tester.pump();
    expect(seeks.last, lessThan(0.02));
  });
}
