import 'dart:math' as math;
import 'dart:ui';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../api/client.dart';
import '../design/accent.dart';
import '../design/components.dart';
import '../design/tokens.dart';
import '../player/controller.dart';
import '../player/likes.dart';
import '../widgets/queue_sheet.dart';
import '../widgets/waveform.dart';

class PlayerScreen extends StatelessWidget {
  const PlayerScreen({
    super.key,
    required this.player,
    required this.api,
    required this.likes,
    required this.onRadio,
  });
  final PlayerController player;
  final Api api;
  final LikesStore likes;
  final void Function(Track) onRadio;

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: player,
      builder: (context, _) {
        final t = player.current;
        if (t == null) return const SizedBox.shrink();
        final p = Palette.of(context);
        final size = MediaQuery.sizeOf(context);
        final wide = size.width >= 900;
        final art = wide ? 340.0 : (size.width * 0.72).clamp(180.0, 340.0);
        final coverUrl = api.cover(t.albumId, t.hasCover);

        return Scaffold(
          backgroundColor: p.room,
          body: CallbackShortcuts(
            bindings: {
              const SingleActivator(LogicalKeyboardKey.escape): () =>
                  Navigator.of(context).maybePop(),
              const SingleActivator(LogicalKeyboardKey.space): player.toggle,
              const SingleActivator(LogicalKeyboardKey.arrowRight): () =>
                  player.seek(player.progress + 0.02),
              const SingleActivator(LogicalKeyboardKey.arrowLeft): () =>
                  player.seek(player.progress - 0.02),
            },
            child: Focus(
              autofocus: true,
              child: Stack(
                fit: StackFit.expand,
                children: [
                  _RoomWash(url: coverUrl, room: p.room),
                  SafeArea(
                    child: Column(
                      children: [
                        _TopBar(
                          player: player,
                          api: api,
                          likes: likes,
                          p: p,
                          track: t,
                          onRadio: onRadio,
                        ),
                        Expanded(
                          child: Center(
                            child: SingleChildScrollView(
                              padding: const EdgeInsets.symmetric(
                                horizontal: S.xl,
                                vertical: S.lg,
                              ),
                              child: ConstrainedBox(
                                constraints: BoxConstraints(maxWidth: art),
                                child: Column(
                                  mainAxisSize: MainAxisSize.min,
                                  children: [
                                    Hero(
                                      tag: 'now-cover',
                                      child: Container(
                                        decoration: BoxDecoration(
                                          borderRadius: BorderRadius.circular(
                                            14,
                                          ),
                                          boxShadow: const [
                                            BoxShadow(
                                              color: Color(0x66000000),
                                              blurRadius: 44,
                                              offset: Offset(0, 18),
                                            ),
                                          ],
                                        ),
                                        child: CoverArt(
                                          key: ValueKey('cover-${t.id}'),
                                          url: coverUrl,
                                          size: art,
                                          radius: 14,
                                          border: false,
                                          // The Hero flight is the transition;
                                          // a second reveal would show the
                                          // finished image and then animate it.
                                          animate: false,
                                        ),
                                      ),
                                    ),
                                    const SizedBox(height: S.xxl),
                                    ScrambleText(
                                      t.title,
                                      key: ValueKey('title-${t.id}'),
                                      style: wide
                                          ? T.displayL(p.onRoom)
                                          : T.displayM(p.onRoom),
                                    ),
                                    const SizedBox(height: 2),
                                    Text(
                                      t.artist,
                                      textAlign: TextAlign.center,
                                      maxLines: 1,
                                      overflow: TextOverflow.ellipsis,
                                      style: T.body(p.onRoomMute),
                                    ),
                                    const SizedBox(height: S.xl),
                                    FutureBuilder<Color>(
                                      future: AccentCache.of(coverUrl),
                                      builder: (context, snap) {
                                        final accent =
                                            snap.data ?? AccentCache.fallback;
                                        return Column(
                                          children: [
                                            Padding(
                                              padding: EdgeInsets.symmetric(
                                                horizontal: art * 0.14,
                                              ),
                                              child: TrackWaveform(
                                                api: api,
                                                track: t,
                                                player: player,
                                                accent: accent,
                                                height: wide ? 48 : 40,
                                              ),
                                            ),
                                            const SizedBox(height: S.xs),
                                            FrameTicker(
                                              playing: player.playing,
                                              builder: (context) => Row(
                                                children: [
                                                  Text(
                                                    mmss(player.position),
                                                    style: T.micro(
                                                      p.onRoomMute,
                                                    ),
                                                  ),
                                                  const Spacer(),
                                                  Text(
                                                    mmss(
                                                      player.duration -
                                                          player.position,
                                                    ),
                                                    style: T.micro(
                                                      p.onRoomMute,
                                                    ),
                                                  ),
                                                ],
                                              ),
                                            ),
                                          ],
                                        );
                                      },
                                    ),
                                    const SizedBox(height: S.xl),
                                    _Controls(
                                      player: player,
                                      url: coverUrl,
                                      api: api,
                                      likes: likes,
                                      track: t,
                                    ),
                                  ],
                                ),
                              ),
                            ),
                          ),
                        ),
                      ],
                    ),
                  ),
                ],
              ),
            ),
          ),
        );
      },
    );
  }
}

/// Blurred cover as the room light: the interface inherits the album's colour
/// without ever hard-coding a palette extraction step.
class _RoomWash extends StatelessWidget {
  const _RoomWash({required this.url, required this.room});
  final String? url;
  final Color room;

  @override
  Widget build(BuildContext context) {
    return Stack(
      fit: StackFit.expand,
      children: [
        ColoredBox(color: room),
        if (url != null)
          Opacity(
            opacity: 0.55,
            child: ImageFiltered(
              imageFilter: ImageFilter.blur(sigmaX: 60, sigmaY: 60),
              child: Image.network(url!, fit: BoxFit.cover),
            ),
          ),
        DecoratedBox(
          decoration: BoxDecoration(
            gradient: LinearGradient(
              begin: Alignment.topCenter,
              end: Alignment.bottomCenter,
              colors: [
                room.withValues(alpha: 0.55),
                room.withValues(alpha: 0.86),
                room,
              ],
              stops: const [0, 0.62, 1],
            ),
          ),
        ),
        // Slow drifting orbs in the cover's own colours. They live behind a
        // gradient, stay under 12% opacity and are the only animated thing on
        // this screen, so the cost stays where nobody notices it.
        FutureBuilder<Color>(
          future: AccentCache.of(url),
          builder: (context, snap) =>
              _Blobs(accent: snap.data ?? AccentCache.fallback, room: room),
        ),
      ],
    );
  }
}

class _Blobs extends StatefulWidget {
  const _Blobs({required this.accent, required this.room});
  final Color accent;
  final Color room;

  @override
  State<_Blobs> createState() => _BlobsState();
}

class _BlobsState extends State<_Blobs> with SingleTickerProviderStateMixin {
  late final AnimationController _c = AnimationController(
    vsync: this,
    duration: const Duration(seconds: 34),
  )..repeat();

  @override
  void dispose() {
    _c.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return IgnorePointer(
      child: RepaintBoundary(
        child: AnimatedBuilder(
          animation: _c,
          builder: (context, _) => CustomPaint(
            painter: _BlobPainter(
              t: _c.value,
              accent: widget.accent,
              room: widget.room,
            ),
            size: Size.infinite,
          ),
        ),
      ),
    );
  }
}

class _BlobPainter extends CustomPainter {
  _BlobPainter({required this.t, required this.accent, required this.room});
  final double t;
  final Color accent;
  final Color room;

  @override
  void paint(Canvas canvas, Size size) {
    if (size.isEmpty) return;
    final shortest = size.shortestSide;
    void orb(double phase, double cx, double cy, double radius, Color color) {
      final a = (t + phase) * 2 * 3.1415926;
      final center = Offset(
        cx * size.width + math.sin(a) * size.width * 0.06,
        cy * size.height + math.cos(a * 0.8) * size.height * 0.05,
      );
      final paint = Paint()
        ..color = color.withValues(alpha: 0.10)
        ..maskFilter = MaskFilter.blur(BlurStyle.normal, shortest * 0.16);
      canvas.drawCircle(center, radius * shortest, paint);
    }

    orb(0.0, 0.28, 0.22, 0.34, accent);
    orb(0.37, 0.74, 0.36, 0.26, accent);
    orb(0.63, 0.52, 0.78, 0.30, Color.lerp(accent, Colors.white, 0.35)!);
  }

  @override
  bool shouldRepaint(_BlobPainter old) =>
      old.t != t || old.accent != accent || old.room != room;
}

class _TopBar extends StatelessWidget {
  const _TopBar({
    required this.player,
    required this.api,
    required this.likes,
    required this.p,
    required this.track,
    required this.onRadio,
  });

  final PlayerController player;
  final Api api;
  final LikesStore likes;
  final Palette p;
  final Track track;
  final void Function(Track) onRadio;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(S.md, S.sm, S.md, 0),
      child: Row(
        children: [
          IconButton(
            icon: Icon(Icons.keyboard_arrow_down_rounded, color: p.onRoom),
            onPressed: () => Navigator.of(context).maybePop(),
          ),
          const Spacer(),
          const Spacer(),
          IconButton(
            icon: Icon(Icons.more_vert_rounded, color: p.onRoom),
            onPressed: () =>
                _openMenu(context, player, api, likes, track, onRadio),
          ),
        ],
      ),
    );
  }
}

void _openMenu(
  BuildContext context,
  PlayerController player,
  Api api,
  LikesStore likes,
  Track track,
  void Function(Track) onRadio,
) {
  final p = Palette.of(context);
  showModalBottomSheet<void>(
    context: context,
    backgroundColor: p.roomElevated,
    showDragHandle: true,
    shape: const RoundedRectangleBorder(
      borderRadius: BorderRadius.vertical(top: Radius.circular(20)),
    ),
    builder: (context) => SafeArea(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          _MenuRow(
            icon: Icons.auto_awesome_rounded,
            label: 'радио от этого трека',
            onTap: () {
              Navigator.of(context).maybePop();
              onRadio(track);
            },
          ),
          _MenuRow(
            icon: Icons.queue_music_rounded,
            label: 'открыть очередь',
            onTap: () {
              Navigator.of(context).maybePop();
              showQueue(context, player: player, api: api, likes: likes);
            },
          ),
          _MenuRow(
            icon: Icons.download_rounded,
            label: 'download · ${track.format}',
          ),
          _MenuRow(
            icon: Icons.info_outline_rounded,
            label: '${track.album} · ${track.year ?? '—'}',
          ),
        ],
      ),
    ),
  );
}

class _MenuRow extends StatelessWidget {
  const _MenuRow({required this.icon, required this.label, this.onTap});
  final IconData icon;
  final String label;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    return ListTile(
      dense: true,
      leading: Icon(icon, size: 20, color: p.onRoomMute),
      title: Text(label, style: T.body(p.onRoom)),
      onTap: onTap ?? () => Navigator.of(context).maybePop(),
    );
  }
}

class _Controls extends StatelessWidget {
  const _Controls({
    required this.player,
    required this.url,
    required this.api,
    required this.likes,
    required this.track,
  });
  final PlayerController player;
  final String? url;
  final Api api;
  final LikesStore likes;
  final Track track;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    return FutureBuilder<Color>(
      future: AccentCache.of(url),
      builder: (context, snap) {
        final accent = snap.data ?? AccentCache.fallback;
        // Equal flex on both sides keeps the play button at the exact centre;
        // a plain centred Row drifts left as soon as the sides differ in width.
        return Row(
          children: [
            Expanded(
              child: Align(
                alignment: Alignment.centerRight,
                child: IconButton(
                  iconSize: 30,
                  icon: Icon(Icons.skip_previous_rounded, color: p.onRoom),
                  onPressed: player.previous,
                ),
              ),
            ),
            const SizedBox(width: S.lg),
            GestureDetector(
              onTap: player.toggle,
              child: AnimatedContainer(
                duration: const Duration(milliseconds: 160),
                width: 68,
                height: 68,
                decoration: BoxDecoration(
                  color: accent,
                  shape: BoxShape.circle,
                  boxShadow: [
                    BoxShadow(
                      color: accent.withValues(alpha: 0.35),
                      blurRadius: 26,
                      offset: const Offset(0, 8),
                    ),
                  ],
                ),
                child: Icon(
                  player.playing
                      ? Icons.pause_rounded
                      : Icons.play_arrow_rounded,
                  size: 38,
                  color: Colors.white,
                ),
              ),
            ),
            const SizedBox(width: S.lg),
            Expanded(
              child: Row(
                mainAxisAlignment: MainAxisAlignment.start,
                children: [
                  IconButton(
                    iconSize: 30,
                    icon: Icon(Icons.skip_next_rounded, color: p.onRoom),
                    onPressed: player.next,
                  ),
                  const SizedBox(width: S.lg),
                  GestureDetector(
                    onTap: () => likes.toggle(track),
                    child: AnimatedScale(
                      scale: likes.contains(track.id) ? 1.0 : 0.9,
                      duration: const Duration(milliseconds: 160),
                      curve: Curves.easeOut,
                      child: Icon(
                        likes.contains(track.id)
                            ? Icons.favorite_rounded
                            : Icons.favorite_border_rounded,
                        size: 24,
                        color: likes.contains(track.id) ? accent : p.onRoomMute,
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ],
        );
      },
    );
  }
}
