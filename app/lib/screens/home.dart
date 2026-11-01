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
    this.size = 160,
  });

  final Album album;
  final String? coverUrl;
  final VoidCallback onPlay;
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

class HomeScreen extends StatelessWidget {
  const HomeScreen({
    super.key,
    required this.tracks,
    required this.api,
    required this.player,
  });

  final List<Track> tracks;
  final Api api;
  final PlayerController player;

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
    final albums = albumsOf(tracks);
    final recent = tracks.take(12).toList();

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
                        '${tracks.length} tracks · ${albums.length} albums',
                        style: T.caption(p.mute),
                      ),
                    ],
                  ),
                ),
                PillButton(
                  icon: Icons.shuffle_rounded,
                  label: 'shuffle',
                  onTap: tracks.isEmpty
                      ? () {}
                      : () => player.play(tracks.first, queue: tracks),
                ),
              ],
            ),
          ),
        ),
        if (recent.isNotEmpty) ...[
          SliverToBoxAdapter(
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: S.xl),
              child: _SectionLabel('recently added', '${recent.length}'),
            ),
          ),
          SliverToBoxAdapter(
            child: SizedBox(
              height: 228,
              child: ListView.separated(
                scrollDirection: Axis.horizontal,
                padding: const EdgeInsets.fromLTRB(S.xl, S.sm, S.xl, S.md),
                itemCount: albums.take(8).length,
                separatorBuilder: (_, _) => const SizedBox(width: S.lg),
                itemBuilder: (context, i) {
                  final a = albums[i];
                  return AlbumCard(
                    album: a,
                    coverUrl: api.cover(a.id, a.hasCover),
                    size: 150,
                    onPlay: () => player.play(a.tracks.first, queue: tracks),
                  );
                },
              ),
            ),
          ),
        ],
        SliverToBoxAdapter(
          child: Padding(
            padding: const EdgeInsets.fromLTRB(S.xl, S.lg, S.xl, 0),
            child: Column(
              children: [
                _SectionLabel('albums', '${albums.length}'),
                const SizedBox(height: S.sm),
              ],
            ),
          ),
        ),
        SliverPadding(
          padding: const EdgeInsets.fromLTRB(S.xl, S.sm, S.xl, S.xxl),
          sliver: SliverGrid(
            gridDelegate: const SliverGridDelegateWithMaxCrossAxisExtent(
              maxCrossAxisExtent: 190,
              mainAxisExtent: 224,
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
                  coverUrl: api.cover(a.id, a.hasCover),
                  onPlay: () => player.play(a.tracks.first, queue: tracks),
                ),
              );
            }, childCount: albums.length),
          ),
        ),
      ],
    );
  }
}

class _SectionLabel extends StatelessWidget {
  const _SectionLabel(this.label, this.count);
  final String label;
  final String count;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    return Row(
      crossAxisAlignment: CrossAxisAlignment.end,
      children: [
        Text(label, style: T.heading(p.ink)),
        const SizedBox(width: S.sm),
        Text(count, style: T.caption(p.ash)),
        const SizedBox(width: S.md),
        Expanded(child: Hr(color: p.hairline)),
      ],
    );
  }
}
