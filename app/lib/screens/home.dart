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
      final url = widget.api.cover(t.albumId, t.hasCover, w: 128);
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
    required this.onOpenLibrary,
  });

  final List<Track> tracks;
  final Api api;
  final PlayerController player;
  final void Function(Track) onRadio;
  final Future<void> Function() onRescan;
  final Color accent;
  final void Function(Collection) onOpen;
  final void Function(int albumId) onOpenAlbum;
  final VoidCallback onOpenLibrary;

  @override
  State<HomeScreen> createState() => _HomeScreenState();
}

/// "Pick up where you left off", across devices: the server holds one cursor
/// per listener, so this card appears on the phone after the desktop stopped
/// halfway through a track.
class _ResumeCard extends StatelessWidget {
  const _ResumeCard({
    required this.cursor,
    required this.api,
    required this.player,
  });

  final PlaybackCursor cursor;
  final Api api;
  final PlayerController player;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    final accent = AccentScope.of(context);
    final t = cursor.track!;
    final left = t.duration - cursor.position;
    return GestureDetector(
      onTap: () => player.play(t, startAt: cursor.position),
      child: MouseRegion(
        cursor: SystemMouseCursors.click,
        child: Container(
          padding: const EdgeInsets.all(S.md),
          decoration: BoxDecoration(
            color: p.soft,
            border: Border.all(color: p.hairline),
            borderRadius: BorderRadius.circular(14),
          ),
          child: Row(
            children: [
              CoverArt(url: api.cover(t.albumId, t.hasCover, w: 128), size: 52),
              const SizedBox(width: S.md),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      'продолжить · ${_clock(cursor.position)}',
                      style: T.micro(accent),
                    ),
                    const SizedBox(height: 2),
                    Text(
                      t.title,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: T.heading(p.ink),
                    ),
                    Text(
                      '${t.artist} · осталось ${_clock(left)}',
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: T.caption(p.mute),
                    ),
                  ],
                ),
              ),
              Icon(Icons.play_arrow_rounded, color: accent),
            ],
          ),
        ),
      ),
    );
  }

  static String _clock(Duration d) {
    final m = d.inMinutes;
    final s = d.inSeconds % 60;
    return '$m:${s.toString().padLeft(2, '0')}';
  }
}

class _HomeScreenState extends State<HomeScreen> {
  List<Mix> _mixes = const [];
  List<StatTrack> _frequent = const [];
  List<Track> _recent = const [];
  PlaybackCursor? _resume;

  @override
  void initState() {
    super.initState();
    _loadMixes();
    _loadRows();
    _loadCursor();
  }

  Future<void> _loadCursor() async {
    try {
      final c = await widget.api.cursor();
      if (!mounted) return;
      setState(() => _resume = c != null && c.resumable ? c : null);
    } catch (_) {}
  }

  /// "Caught often" is the honest name for the first row: the tracks the
  /// listener actually replays, and the newest additions when nothing has been
  /// played yet, so the row is never empty on a fresh library.
  Future<void> _loadRows() async {
    try {
      final stats = await widget.api.stats();
      if (!mounted) return;
      setState(() {
        _frequent = stats.topTracks.take(7).toList();
        _recent = widget.tracks.take(7).toList();
      });
    } catch (_) {
      if (mounted) setState(() => _recent = widget.tracks.take(7).toList());
    }
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
        if (_resume != null && widget.player.current?.id != _resume!.trackId)
          SliverToBoxAdapter(
            child: Padding(
              padding: const EdgeInsets.fromLTRB(S.xl, S.lg, S.xl, 0),
              child: _ResumeCard(
                cursor: _resume!,
                api: widget.api,
                player: widget.player,
              ),
            ),
          ),
        if (_frequent.isNotEmpty || _recent.isNotEmpty)
          SliverToBoxAdapter(
            child: Padding(
              padding: const EdgeInsets.fromLTRB(S.xl, S.lg, S.xl, 0),
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.end,
                children: [
                  Text(
                    _frequent.isEmpty ? 'новое в библиотеке' : 'часто слушаешь',
                    style: T.heading(p.ink),
                  ),
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
        if (_frequent.isNotEmpty || _recent.isNotEmpty)
          SliverPadding(
            padding: const EdgeInsets.fromLTRB(S.xl, S.sm, S.xl, 0),
            sliver: SliverList(
              delegate: SliverChildBuilderDelegate(
                (context, i) {
                  if (_frequent.isNotEmpty) {
                    final t = _frequent[i];
                    final tracks = _frequent
                        .map(
                          (e) => Track(
                            id: e.id,
                            title: e.title,
                            artist: e.artist,
                            album: '',
                            albumId: e.albumId,
                            hasCover: e.hasCover,
                          ),
                        )
                        .toList();
                    return _FrequentRow(
                      index: i,
                      title: t.title,
                      artist: t.artist,
                      plays: t.plays,
                      coverUrl: widget.api.cover(t.albumId, t.hasCover, w: 128),
                      onPlay: () =>
                          widget.player.play(tracks[i], queue: tracks),
                      onRadio: () => widget.onRadio(tracks[i]),
                    );
                  }
                  final t = _recent[i];
                  return _FrequentRow(
                    index: i,
                    title: t.title,
                    artist: t.artist,
                    plays: 0,
                    coverUrl: widget.api.cover(t.albumId, t.hasCover, w: 128),
                    onPlay: () => widget.player.play(t, queue: _recent),
                    onRadio: () => widget.onRadio(t),
                  );
                },
                childCount: _frequent.isNotEmpty
                    ? _frequent.length
                    : _recent.length,
              ),
            ),
          ),
        SliverToBoxAdapter(
          child: Padding(
            padding: const EdgeInsets.fromLTRB(S.xl, S.xxl, S.xl, 0),
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
                      final url = widget.api.cover(
                        t.albumId,
                        t.hasCover,
                        w: 128,
                      );
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
                const SizedBox(width: S.md),
                Tooltip(
                  message: 'все альбомы',
                  child: MouseRegion(
                    cursor: SystemMouseCursors.click,
                    child: GestureDetector(
                      onTap: widget.onOpenLibrary,
                      child: MouseRegion(
                        cursor: SystemMouseCursors.click,
                        child: Padding(
                          padding: const EdgeInsets.only(bottom: 3),
                          child: Icon(
                            Icons.arrow_forward_rounded,
                            size: 18,
                            color: p.mute,
                          ),
                        ),
                      ),
                    ),
                  ),
                ),
              ],
            ),
          ),
        ),
        SliverToBoxAdapter(
          child: SizedBox(
            height: 214,
            child: ListView.separated(
              scrollDirection: Axis.horizontal,
              padding: const EdgeInsets.fromLTRB(S.xl, S.lg, S.xl, 0),
              itemCount: albums.length,
              separatorBuilder: (_, _) => const SizedBox(width: S.md),
              itemBuilder: (context, i) {
                final album = albums[i];
                return SizedBox(
                  width: 150,
                  child: StaggerIn(
                    key: ValueKey('album-${album.id}'),
                    index: i,
                    child: GestureDetector(
                      onTap: () => widget.onOpenAlbum(album.id),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          ClipRRect(
                            borderRadius: BorderRadius.circular(10),
                            child: CoverArt(
                              url: widget.api.cover(
                                album.id,
                                album.hasCover,
                                w: 320,
                              ),
                              size: 150,
                              radius: 10,
                              animate: false,
                            ),
                          ),
                          const SizedBox(height: S.sm),
                          Text(
                            album.title,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: T.bodyStrong(p.ink),
                          ),
                          Text(
                            '${album.artist} · ${album.year ?? '—'}',
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: T.micro(p.mute),
                          ),
                        ],
                      ),
                    ),
                  ),
                );
              },
            ),
          ),
        ),
        SliverPadding(
          padding: const EdgeInsets.fromLTRB(S.xl, 0, S.xl, S.xxl),
          sliver: const SliverToBoxAdapter(child: SizedBox(height: S.xs)),
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

/// One line of the "caught often" column: rank, cover, title, play count.
/// Deliberately a list and not a card grid — a column of tracks reads faster
/// than nine tiles and matches how the rest of the app counts things.
class _FrequentRow extends StatefulWidget {
  const _FrequentRow({
    required this.index,
    required this.title,
    required this.artist,
    required this.plays,
    required this.coverUrl,
    required this.onPlay,
    required this.onRadio,
  });

  final int index;
  final String title;
  final String artist;
  final int plays;
  final String? coverUrl;
  final VoidCallback onPlay;
  final VoidCallback onRadio;

  @override
  State<_FrequentRow> createState() => _FrequentRowState();
}

class _FrequentRowState extends State<_FrequentRow> {
  bool _hover = false;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    final accent = AccentScope.of(context);
    return MouseRegion(
      cursor: SystemMouseCursors.click,
      onEnter: (_) => setState(() => _hover = true),
      onExit: (_) => setState(() => _hover = false),
      child: GestureDetector(
        onTap: widget.onPlay,
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 140),
          padding: const EdgeInsets.symmetric(horizontal: S.sm, vertical: 6),
          decoration: BoxDecoration(
            color: _hover ? p.soft : Colors.transparent,
            borderRadius: BorderRadius.circular(10),
          ),
          child: Row(
            children: [
              SizedBox(
                width: 24,
                child: Text(
                  '${widget.index + 1}',
                  style: T.caption(_hover ? accent : p.ash),
                ),
              ),
              CoverArt(
                url: widget.coverUrl,
                size: 38,
                radius: 7,
                animate: false,
              ),
              const SizedBox(width: S.md),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      widget.title,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: T.bodyStrong(p.ink),
                    ),
                    Text(
                      widget.artist,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: T.micro(p.mute),
                    ),
                  ],
                ),
              ),
              if (widget.plays > 0)
                Padding(
                  padding: const EdgeInsets.only(right: S.md),
                  child: Text('${widget.plays}×', style: T.micro(p.ash)),
                ),
              AnimatedOpacity(
                duration: const Duration(milliseconds: 140),
                opacity: _hover ? 1 : 0,
                child: Icon(Icons.play_arrow_rounded, size: 20, color: accent),
              ),
              const SizedBox(width: S.sm),
              AnimatedOpacity(
                duration: const Duration(milliseconds: 140),
                opacity: _hover ? 1 : 0,
                child: GestureDetector(
                  onTap: widget.onRadio,
                  child: Icon(Icons.radio_rounded, size: 18, color: p.mute),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
