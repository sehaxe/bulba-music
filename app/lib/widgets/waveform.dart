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
    required this.player,
    required this.accent,
    this.height = 72,
  });

  final Api api;
  final Track track;
  final PlayerController player;
  final Color accent;
  final double height;

  @override
  State<TrackWaveform> createState() => _TrackWaveformState();
}

class _TrackWaveformState extends State<TrackWaveform>
    with SingleTickerProviderStateMixin {
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
    if (peaks == null) {
      return SizedBox(height: widget.height);
    }
    if (peaks.isEmpty) {
      return SizedBox(
        height: widget.height,
        child: Center(
          child: Text('[ no waveform ]', style: T.micro(p.onRoomMute)),
        ),
      );
    }
    return FrameTicker(
      playing: widget.player.playing,
      builder: (context) => LayoutBuilder(
        builder: (context, box) {
          // Measure against the real render box and convert the pointer, so a
          // tap lands where the finger is no matter how the row is padded.
          void seek(Offset global) {
            final object = context.findRenderObject();
            if (object is! RenderBox || object.size.width <= 0) return;
            final local = object.globalToLocal(global);
            widget.player.seek((local.dx / object.size.width).clamp(0.0, 1.0));
          }

          return GestureDetector(
            behavior: HitTestBehavior.opaque,
            onTapDown: (d) => seek(d.globalPosition),
            onHorizontalDragUpdate: (d) => seek(d.globalPosition),
            child: AnimatedBuilder(
              animation: _reveal,
              builder: (context, _) => ClipRect(
                child: CustomPaint(
                  size: Size(box.maxWidth, widget.height),
                  painter: _WavePainter(
                    peaks: peaks,
                    progress: widget.player.progress,
                    accent: widget.accent,
                    base: p.onRoom.withValues(alpha: 0.18),
                    reveal: Curves.easeOutCubic.transform(_reveal.value),
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

  @override
  void paint(Canvas canvas, Size size) {
    final n = peaks.length;
    if (n == 0) return;
    final gap = math.max(1.0, size.width / n * 0.38);
    final barW = math.max(1.0, (size.width - gap * (n - 1)) / n);
    final mid = size.height / 2;
    final playedPaint = Paint()..color = accent;
    final restPaint = Paint()..color = base;

    for (var i = 0; i < n; i++) {
      final v = (peaks[i] / 255.0).clamp(0.0, 1.0);
      // Half-height: the bar is mirrored around the centre, so the total is 2h.
      final h = math.max(1.0, v * (size.height / 2) * 0.92 * reveal);
      final x = i * (barW + gap);
      final played = (i + 0.5) / n <= progress;
      final rrect = RRect.fromRectAndRadius(
        Rect.fromLTWH(x, mid - h, barW, h * 2),
        Radius.circular(barW / 2),
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
