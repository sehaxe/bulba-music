import 'dart:math' as math;

import 'package:flutter/material.dart';

import '../api/client.dart';
import '../design/components.dart';
import '../design/tokens.dart';
import '../player/controller.dart';

/// The waveform is mirrored around a centre line — the "sound bar in both
/// directions" — and doubles as the seek surface: tap to jump, drag to scrub.
class TrackWaveform extends StatefulWidget {
  const TrackWaveform({
    super.key,
    required this.api,
    required this.track,
    required this.progress,
    required this.onSeek,
    required this.accent,
    required this.playing,
    this.height = 72,
  });

  final Api api;
  final Track track;

  /// Current position as a fraction, 0..1.
  final double progress;

  /// Called with the target fraction: tapping and dragging both end up here.
  final void Function(double ratio) onSeek;
  final bool playing;
  final Color accent;
  final double height;

  @override
  State<TrackWaveform> createState() => _TrackWaveformState();
}

class _TrackWaveformState extends State<TrackWaveform>
    with SingleTickerProviderStateMixin {
  final GlobalKey _barKey = GlobalKey();
  List<int>? _peaks;
  late final AnimationController _reveal = AnimationController(
    vsync: this,
    duration: const Duration(milliseconds: 520),
  );

  @override
  void initState() {
    super.initState();
    _load();
  }

  @override
  void didUpdateWidget(TrackWaveform old) {
    super.didUpdateWidget(old);
    if (old.track.id != widget.track.id) {
      _load();
    }
  }

  @override
  void dispose() {
    _reveal.dispose();
    super.dispose();
  }

  Future<void> _load() async {
    setState(() => _peaks = null);
    try {
      final peaks = await widget.api.peaks(widget.track.id);
      if (!mounted) return;
      setState(() => _peaks = peaks);
      _reveal.forward(from: 0);
    } catch (_) {
      if (!mounted) return;
      setState(() => _peaks = const []);
    }
  }

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    final peaks = _peaks;
    return FrameTicker(
      playing: widget.playing,
      builder: (context) => LayoutBuilder(
        builder: (context, box) {
          // The seek surface is drawn whether or not the peaks arrived: a
          // failing peaks request must not make the player unseekable.
          void seek(Offset local) {
            final object = _barKey.currentContext?.findRenderObject();
            if (object is! RenderBox || object.size.width <= 0) return;
            final ratio = (local.dx / object.size.width).clamp(0.0, 1.0);
            widget.onSeek(ratio);
          }

          return GestureDetector(
            key: _barKey,
            behavior: HitTestBehavior.opaque,
            onTapDown: (d) => seek(d.localPosition),
            onHorizontalDragUpdate: (d) => seek(d.localPosition),
            child: SizedBox(
              height: widget.height,
              width: double.infinity,
              child: peaks == null || peaks.isEmpty
                  ? Center(
                      child: Container(
                        height: 2,
                        color: p.onRoomMute.withValues(alpha: 0.25),
                      ),
                    )
                  : AnimatedBuilder(
                      animation: _reveal,
                      builder: (context, _) => ClipRect(
                        child: CustomPaint(
                          size: Size(box.maxWidth, widget.height),
                          painter: _WavePainter(
                            peaks: peaks,
                            progress: widget.progress,
                            accent: widget.accent,
                            base: p.onRoom.withValues(alpha: 0.18),
                            reveal: Curves.easeOutCubic.transform(
                              _reveal.value,
                            ),
                          ),
                        ),
                      ),
                    ),
            ),
          );
        },
      ),
    );
  }
}

/// Exposed so a test can check the arithmetic without a canvas.
class WaveBarGeometry {
  const WaveBarGeometry(this.barW, this.gap);
  final double barW;
  final double gap;

  static WaveBarGeometry of(int n, double width) {
    final gap = (width / n) * 0.38;
    return WaveBarGeometry(width / n - gap, gap);
  }
}

class _WavePainter extends CustomPainter {
  _WavePainter({
    required this.peaks,
    required this.progress,
    required this.accent,
    required this.base,
    required this.reveal,
  });

  final List<int> peaks;
  final double progress;
  final Color accent;
  final Color base;
  final double reveal;

  /// Bars and gaps that exactly fill [width] for [n] peaks.
  ///
  /// The old version clamped both to at least one pixel, so a 250 px box with
  /// 400 peaks drew 800 px worth of bars and clipped half the track: the
  /// visible waveform only covered the first half of the song, which is why a
  /// click appeared to land to the right of the cursor.
  @override
  void paint(Canvas canvas, Size size) {
    final n = peaks.length;
    if (n == 0) return;
    final g = WaveBarGeometry.of(n, size.width);
    final mid = size.height / 2;
    final playedPaint = Paint()..color = accent;
    final restPaint = Paint()..color = base;

    for (var i = 0; i < n; i++) {
      final v = (peaks[i] / 255.0).clamp(0.0, 1.0);
      // Half-height: the bar is mirrored around the centre, so the total is 2h.
      final h = math.max(1.0, v * (size.height / 2) * 0.92 * reveal);
      final x = i * (g.barW + g.gap);
      final played = (i + 0.5) / n <= progress;
      final w = math.max(0.7, g.barW);
      final rrect = RRect.fromRectAndRadius(
        Rect.fromLTWH(x, mid - h, w, h * 2),
        Radius.circular(w / 2),
      );
      canvas.drawRRect(rrect, played ? playedPaint : restPaint);
    }
  }

  @override
  bool shouldRepaint(_WavePainter old) =>
      old.progress != progress ||
      old.reveal != reveal ||
      old.accent != accent ||
      old.peaks != peaks;
}
