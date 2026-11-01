import 'package:flutter/material.dart';

import '../api/client.dart';
import '../design/components.dart';
import '../design/tokens.dart';
import '../player/controller.dart';

class AlbumCard extends StatefulWidget {
  const AlbumCard({
    super.key,
    required this.album,
    required this.coverUrl,
    required this.onPlay,
    this.onRadio,
    this.size = 160,
  });

  final Album album;
  final String? coverUrl;
  final VoidCallback onPlay;
  final VoidCallback? onRadio;
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
        onTap: widget.onPlay,
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

/// A mix is a horizontal rail of tracks. Playing from it replaces the queue
/// with the whole mix, so "next" stays inside the mood the mix set.
class MixRail extends StatelessWidget {
  const MixRail({
    super.key,
    required this.mix,
    required this.api,
    required this.player,
  });

  final Mix mix;
  final Api api;
  final PlayerController player;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(S.xl, S.xl, S.xl, 0),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.end,
            children: [
              Text(mix.title, style: T.heading(p.ink)),
              const SizedBox(width: S.sm),
              Text('${mix.tracks.length}', style: T.caption(p.ash)),
              const SizedBox(width: S.md),
              Expanded(
                child: Padding(
                  padding: const EdgeInsets.only(bottom: 3),
                  child: Text(mix.note, style: T.micro(p.ash)),
                ),
              ),
            ],
          ),
        ),
        SizedBox(
          height: 216,
          child: ListView.separated(
            scrollDirection: Axis.horizontal,
            padding: const EdgeInsets.fromLTRB(S.xl, S.md, S.xl, S.sm),
            itemCount: mix.tracks.length,
            separatorBuilder: (_, _) => const SizedBox(width: S.md),
            itemBuilder: (context, i) {
              final t = mix.tracks[i];
              return _TrackCard(
                track: t,
                coverUrl: api.cover(t.albumId, t.hasCover),
                onPlay: () => player.play(t, queue: mix.tracks),
              );
            },
          ),
        ),
      ],
    );
  }
}

class _TrackCard extends StatefulWidget {
  const _TrackCard({
    required this.track,
    required this.coverUrl,
    required this.onPlay,
  });
  final Track track;
  final String? coverUrl;
  final VoidCallback onPlay;

  @override
  State<_TrackCard> createState() => _TrackCardState();
}

class _TrackCardState extends State<_TrackCard> {
  bool _hover = false;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    return MouseRegion(
      cursor: SystemMouseCursors.click,
      onEnter: (_) => setState(() => _hover = true),
      onExit: (_) => setState(() => _hover = false),
      child: GestureDetector(
        onTap: widget.onPlay,
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 160),
          curve: Curves.easeOut,
          width: 148,
          padding: const EdgeInsets.all(S.sm),
          decoration: BoxDecoration(
            color: _hover ? p.soft : Colors.transparent,
            borderRadius: BorderRadius.circular(12),
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            mainAxisSize: MainAxisSize.min,
            children: [
              CoverArt(url: widget.coverUrl, size: 132, radius: 8),
              const SizedBox(height: S.sm),
              Text(
                widget.track.title,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: T.bodyStrong(p.ink),
              ),
              Text(
                widget.track.artist,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: T.micro(p.mute),
              ),
            ],
          ),
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
  });

  final List<Track> tracks;
  final Api api;
  final PlayerController player;
  final void Function(Track) onRadio;
  final Future<void> Function() onRescan;

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
      final mixes = await widget.api.homeMixes();
      if (mounted) setState(() => _mixes = mixes);
    } catch (_) {
      // A fresh library may have no mixes yet; the albums grid still stands.
    }
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
    final mixes = _mixes;

    return CustomScrollView(
      slivers: [
        SliverToBoxAdapter(
          child: Padding(
            padding: const EdgeInsets.fromLTRB(S.xl, S.xxl, S.xl, S.lg),
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
                        '${widget.tracks.length} tracks · ${albums.length} albums',
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
                PillButton(
                  icon: Icons.shuffle_rounded,
                  label: 'shuffle',
                  onTap: widget.tracks.isEmpty
                      ? () {}
                      : () => widget.player.play(
                          widget.tracks.first,
                          queue: widget.tracks,
                        ),
                ),
              ],
            ),
          ),
        ),
        for (final mix in mixes)
          SliverToBoxAdapter(
            child: MixRail(mix: mix, api: widget.api, player: widget.player),
          ),
        SliverToBoxAdapter(
          child: Padding(
            padding: const EdgeInsets.fromLTRB(S.xl, S.xxl, S.xl, 0),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.end,
              children: [
                Text('albums', style: T.heading(p.ink)),
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
                ),
              );
            }, childCount: albums.length),
          ),
        ),
      ],
    );
  }
}
