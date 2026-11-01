import 'package:flutter/material.dart';

import '../api/client.dart';
import '../design/components.dart';
import '../design/tokens.dart';
import '../player/controller.dart';
import 'collection.dart';

/// A daily mix card: a mosaic of the covers inside it, the number, and what the
/// mix sounds like. One decision per card — press it and the whole playlist
/// becomes the queue.
class DailyCard extends StatefulWidget {
  const DailyCard({
    super.key,
    required this.mix,
    required this.api,
    required this.player,
    required this.accent,
    required this.onOpen,
  });

  final Mix mix;
  final Api api;
  final PlayerController player;
  final Color accent;
  final VoidCallback onOpen;

  @override
  State<DailyCard> createState() => _DailyCardState();
}

class _DailyCardState extends State<DailyCard> {
  bool _hover = false;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    final covers = <String?>[];
    for (final t in widget.mix.tracks) {
      final url = widget.api.cover(t.albumId, t.hasCover);
      if (url != null && !covers.contains(url)) covers.add(url);
      if (covers.length == 4) break;
    }

    return MouseRegion(
      cursor: SystemMouseCursors.click,
      onEnter: (_) => setState(() => _hover = true),
      onExit: (_) => setState(() => _hover = false),
      child: GestureDetector(
        onTap: widget.onOpen,
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 160),
          curve: Curves.easeOut,
          padding: const EdgeInsets.all(S.md),
          decoration: BoxDecoration(
            color: _hover ? p.soft : p.canvas,
            border: Border.all(color: p.hairline),
            borderRadius: BorderRadius.circular(14),
          ),
          child: Row(
            children: [
              _Mosaic(covers: covers, size: 132),
              const SizedBox(width: S.lg),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    Text(
                      widget.mix.title,
                      style: mono(21, 700, p.ink, height: 1.2),
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    ),
                    const SizedBox(height: S.xs),
                    Text(
                      widget.mix.note,
                      style: T.caption(p.mute),
                      maxLines: 2,
                      overflow: TextOverflow.ellipsis,
                    ),
                    const SizedBox(height: S.md),
                    Row(
                      children: [
                        AnimatedContainer(
                          duration: const Duration(milliseconds: 160),
                          width: 40,
                          height: 40,
                          decoration: BoxDecoration(
                            color: _hover ? widget.accent : p.card,
                            shape: BoxShape.circle,
                          ),
                          child: Icon(
                            Icons.play_arrow_rounded,
                            size: 24,
                            color: _hover ? Colors.white : p.ink,
                          ),
                        ),
                        const SizedBox(width: S.sm),
                        Text(
                          '${widget.mix.tracks.length} ${plural(widget.mix.tracks.length)}',
                          style: T.micro(p.ash),
                        ),
                      ],
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _Mosaic extends StatelessWidget {
  const _Mosaic({required this.covers, required this.size});
  final List<String?> covers;
  final double size;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    if (covers.isEmpty) {
      return Hatch(size: size, radius: 10);
    }
    if (covers.length < 3) {
      return CoverArt(url: covers.first, size: size, radius: 10);
    }
    final half = (size - 2) / 2;
    final cells = <Widget>[];
    for (var i = 0; i < 4; i++) {
      final url = i < covers.length ? covers[i] : null;
      cells.add(
        url == null
            ? Container(width: half, height: half, color: p.card)
            : CoverArt(url: url, size: half, radius: 2, animate: false),
      );
    }
    return ClipRRect(
      borderRadius: BorderRadius.circular(10),
      child: SizedBox(
        width: size,
        height: size,
        child: Column(
          children: [
            Row(children: [cells[0], const SizedBox(width: 2), cells[1]]),
            const SizedBox(height: 2),
            Row(children: [cells[2], const SizedBox(width: 2), cells[3]]),
          ],
        ),
      ),
    );
  }
}

class HomeScreen extends StatefulWidget {
  const HomeScreen({
    super.key,
    required this.tracks,
    required this.api,
    required this.player,
    required this.onRadio,
    required this.onRescan,
    required this.accent,
    required this.onOpen,
    required this.onOpenAlbum,
  });

  final List<Track> tracks;
  final Api api;
  final PlayerController player;
  final void Function(Track) onRadio;
  final Future<void> Function() onRescan;
  final Color accent;
  final void Function(Collection) onOpen;
  final void Function(int albumId) onOpenAlbum;

  @override
  State<HomeScreen> createState() => _HomeScreenState();
}

class _HomeScreenState extends State<HomeScreen> {
  List<Mix> _mixes = const [];

  @override
  void initState() {
    super.initState();
    _loadMixes();
  }

  Future<void> _loadMixes() async {
    try {
      var mixes = await widget.api.dailyMixes();
      if (mixes.isEmpty) mixes = await widget.api.homeMixes();
      if (mounted) setState(() => _mixes = mixes);
    } catch (_) {}
  }

  String get _greeting {
    final h = DateTime.now().hour;
    if (h < 6) return 'still awake';
    if (h < 12) return 'good morning';
    if (h < 18) return 'good afternoon';
    return 'good evening';
  }

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    final albums = albumsOf(widget.tracks);

    return CustomScrollView(
      slivers: [
        SliverToBoxAdapter(
          child: Padding(
            padding: const EdgeInsets.fromLTRB(S.xl, S.xxl, S.xl, S.sm),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.end,
              children: [
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(_greeting, style: T.displayM(p.ink)),
                      const SizedBox(height: S.xxs),
                      Text(
                        '${widget.tracks.length} треков · ${albums.length} альбомов',
                        style: T.caption(p.mute),
                      ),
                    ],
                  ),
                ),
                IconButton(
                  tooltip: 'rescan library',
                  onPressed: () async {
                    await widget.onRescan();
                    await _loadMixes();
                  },
                  icon: Icon(Icons.refresh_rounded, size: 20, color: p.mute),
                ),
              ],
            ),
          ),
        ),
        SliverToBoxAdapter(
          child: Padding(
            padding: const EdgeInsets.fromLTRB(S.xl, S.lg, S.xl, 0),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.end,
              children: [
                Text('подборки дня', style: T.heading(p.ink)),
                const SizedBox(width: S.md),
                Expanded(
                  child: Padding(
                    padding: const EdgeInsets.only(bottom: 3),
                    child: Hr(color: p.hairline),
                  ),
                ),
              ],
            ),
          ),
        ),
        SliverPadding(
          padding: const EdgeInsets.fromLTRB(S.xl, S.md, S.xl, S.md),
          sliver: SliverGrid(
            gridDelegate: const SliverGridDelegateWithMaxCrossAxisExtent(
              maxCrossAxisExtent: 460,
              mainAxisExtent: 178,
              crossAxisSpacing: S.md,
              mainAxisSpacing: S.md,
            ),
            delegate: SliverChildBuilderDelegate((context, i) {
              final mix = _mixes[i];
              return StaggerIn(
                key: ValueKey('mix-${mix.id}'),
                index: i,
                child: DailyCard(
                  mix: mix,
                  api: widget.api,
                  player: widget.player,
                  accent: widget.accent,
                  onOpen: () {
                    final covers = <String?>[];
                    for (final t in mix.tracks) {
                      final url = widget.api.cover(t.albumId, t.hasCover);
                      if (url != null && !covers.contains(url)) {
                        covers.add(url);
                      }
                      if (covers.length == 4) break;
                    }
                    widget.onOpen(
                      Collection(
                        title: mix.title,
                        subtitle: mix.note,
                        tracks: mix.tracks,
                        mosaic: covers,
                      ),
                    );
                  },
                ),
              );
            }, childCount: _mixes.length),
          ),
        ),
        SliverToBoxAdapter(
          child: Padding(
            padding: const EdgeInsets.fromLTRB(S.xl, S.xxl, S.xl, 0),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.end,
              children: [
                Text('альбомы', style: T.heading(p.ink)),
                const SizedBox(width: S.sm),
                Text('${albums.length}', style: T.caption(p.ash)),
                const SizedBox(width: S.md),
                Expanded(
                  child: Padding(
                    padding: const EdgeInsets.only(bottom: 3),
                    child: Hr(color: p.hairline),
                  ),
                ),
              ],
            ),
          ),
        ),
        SliverPadding(
          padding: const EdgeInsets.fromLTRB(S.xl, S.lg, S.xl, S.xxl),
          sliver: SliverGrid(
            gridDelegate: const SliverGridDelegateWithMaxCrossAxisExtent(
              maxCrossAxisExtent: 200,
              mainAxisExtent: 232,
              crossAxisSpacing: S.lg,
              mainAxisSpacing: S.xl,
            ),
            delegate: SliverChildBuilderDelegate((context, i) {
              final a = albums[i];
              return StaggerIn(
                key: ValueKey('album-${a.id}'),
                index: i,
                child: AlbumCard(
                  album: a,
                  coverUrl: widget.api.cover(a.id, a.hasCover),
                  onPlay: () =>
                      widget.player.play(a.tracks.first, queue: a.tracks),
                  onRadio: () => widget.onRadio(a.tracks.first),
                  onOpen: () => widget.onOpenAlbum(a.id),
                ),
              );
            }, childCount: albums.length),
          ),
        ),
      ],
    );
  }
}

class AlbumCard extends StatefulWidget {
  const AlbumCard({
    super.key,
    required this.album,
    required this.coverUrl,
    required this.onPlay,
    this.onRadio,
    this.onOpen,
    this.size = 160,
  });

  final Album album;
  final String? coverUrl;
  final VoidCallback onPlay;
  final VoidCallback? onRadio;
  final VoidCallback? onOpen;
  final double size;

  @override
  State<AlbumCard> createState() => _AlbumCardState();
}

class _AlbumCardState extends State<AlbumCard> {
  bool _hover = false;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    final a = widget.album;
    return MouseRegion(
      cursor: SystemMouseCursors.click,
      onEnter: (_) => setState(() => _hover = true),
      onExit: (_) => setState(() => _hover = false),
      child: GestureDetector(
        onTap: widget.onOpen ?? widget.onPlay,
        onLongPress: widget.onRadio,
        child: AnimatedSlide(
          duration: const Duration(milliseconds: 160),
          curve: Curves.easeOut,
          offset: Offset(0, _hover ? -0.02 : 0),
          child: SizedBox(
            width: widget.size,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: [
                CoverArt(
                  url: widget.coverUrl,
                  size: widget.size,
                  radius: 10,
                  animate: true,
                ),
                const SizedBox(height: S.sm),
                Text(
                  a.title,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: T.bodyStrong(p.ink),
                ),
                Text(
                  [
                    a.artist,
                    if (a.year != null) '${a.year}',
                  ].where((e) => e.isNotEmpty).join(' · '),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: T.micro(p.mute),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
