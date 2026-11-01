import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../api/client.dart';
import '../design/components.dart';
import '../design/tokens.dart';
import '../player/controller.dart';
import '../player/likes.dart';
import '../widgets/track_menu.dart';
import '../widgets/track_row.dart';

/// One album: its cover at a size worth looking at, the run time, and the
/// tracks in disc order. The heart here is an album like, separate from track
/// likes — people mean different things by them.
class AlbumScreen extends StatefulWidget {
  const AlbumScreen({
    super.key,
    required this.api,
    required this.player,
    required this.likes,
    required this.albumId,
    required this.onArtist,
    required this.onRadio,
    required this.onBack,
  });

  final Api api;
  final PlayerController player;
  final LikesStore likes;
  final int albumId;
  final void Function(int artistId) onArtist;
  final void Function(Track) onRadio;
  final VoidCallback onBack;

  @override
  State<AlbumScreen> createState() => _AlbumScreenState();
}

class _AlbumScreenState extends State<AlbumScreen> {
  AlbumPage? _page;
  bool _liked = false;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    try {
      final page = await widget.api.albumPage(widget.albumId);
      if (!mounted) return;
      setState(() {
        _page = page;
        _liked = page.album.liked;
      });
    } catch (_) {}
  }

  Future<void> _toggleLike() async {
    final next = !_liked;
    setState(() => _liked = next);
    try {
      await widget.api.likeAlbum(widget.albumId, on: next);
    } catch (_) {
      if (mounted) setState(() => _liked = !next);
    }
  }

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    final page = _page;
    if (page == null) {
      return Center(child: Text('читаю…', style: T.label(p.mute)));
    }
    final album = page.album;
    final tracks = page.tracks;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        SizedBox(
          height: 56,
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: S.sm),
            child: Row(
              children: [
                _Back(onTap: widget.onBack),
                const Spacer(),
              ],
            ),
          ),
        ),
        Expanded(
          child: ListView(
            padding: const EdgeInsets.only(bottom: S.xxl),
            children: [
              Padding(
                padding: const EdgeInsets.fromLTRB(S.xl, S.md, S.xl, S.xl),
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.end,
                  children: [
                    CoverArt(
                      url: widget.api.cover(album.id, album.hasCover),
                      size: 168,
                      radius: 14,
                    ),
                    const SizedBox(width: S.lg),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          GestureDetector(
                            onTap: () => widget.onArtist(album.artistId),
                            child: MouseRegion(
                              cursor: SystemMouseCursors.click,
                              child: Text(
                                album.artist,
                                style: T.label(C.signal),
                              ),
                            ),
                          ),
                          const SizedBox(height: 2),
                          Text(album.title, style: T.displayL(p.ink)),
                          const SizedBox(height: S.xs),
                          Text(
                            [
                              if (album.year != null) '${album.year}',
                              '${tracks.length} ${plural(tracks.length, one: 'трек', few: 'трека', many: 'треков')}',
                              _length(page.totalMs),
                              if (page.plays > 0) '${page.plays} включений',
                            ].join(' · '),
                            style: T.caption(p.mute),
                          ),
                          const SizedBox(height: S.md),
                          Row(
                            children: [
                              PillButton(
                                icon: Icons.play_arrow_rounded,
                                label: 'play',
                                onTap: tracks.isEmpty
                                    ? null
                                    : () => widget.player.play(
                                        tracks.first,
                                        queue: tracks,
                                      ),
                              ),
                              const SizedBox(width: S.sm),
                              _Heart(liked: _liked, onTap: _toggleLike),
                              const SizedBox(width: S.sm),
                              _CopyLink(
                                path: '/al/${widget.albumId}',
                                base: widget.api.base,
                              ),
                            ],
                          ),
                        ],
                      ),
                    ),
                  ],
                ),
              ),
              Hr(color: p.hairlineStrong),
              for (var i = 0; i < tracks.length; i++)
                TrackRow(
                  key: ValueKey('album-${tracks[i].id}'),
                  track: tracks[i],
                  index: i + 1,
                  playing: widget.player.current?.id == tracks[i].id,
                  coverUrl: null,
                  showCover: false,
                  onTap: () => widget.player.play(tracks[i], queue: tracks),
                  onRadio: () => widget.onRadio(tracks[i]),
                  onMenu: () => showTrackMenu(
                    context,
                    track: tracks[i],
                    api: widget.api,
                    player: widget.player,
                    likes: widget.likes,
                    onRadio: () => widget.onRadio(tracks[i]),
                  ),
                  liked: widget.likes.contains(tracks[i].id),
                  onLike: () => widget.likes.toggle(tracks[i]),
                ),
            ],
          ),
        ),
      ],
    );
  }

  static String _length(int ms) {
    final d = Duration(milliseconds: ms);
    if (d.inHours > 0) return '${d.inHours} ч ${d.inMinutes % 60} мин';
    return '${d.inMinutes} мин';
  }
}

class _CopyLink extends StatefulWidget {
  const _CopyLink({required this.path, required this.base});
  final String path;
  final String base;

  @override
  State<_CopyLink> createState() => _CopyLinkState();
}

class _CopyLinkState extends State<_CopyLink> {
  bool _copied = false;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    return GestureDetector(
      onTap: () async {
        await Clipboard.setData(
          ClipboardData(text: '${widget.base}${widget.path}'),
        );
        if (!mounted) return;
        setState(() => _copied = true);
        await Future<void>.delayed(const Duration(seconds: 2));
        if (mounted) setState(() => _copied = false);
      },
      child: MouseRegion(
        cursor: SystemMouseCursors.click,
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 160),
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
          decoration: BoxDecoration(
            color: p.soft,
            border: Border.all(color: p.hairline),
            borderRadius: BorderRadius.circular(9),
          ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(
                _copied ? Icons.check_rounded : Icons.link_rounded,
                size: 15,
                color: _copied ? C.signal : p.mute,
              ),
              const SizedBox(width: 6),
              Text(
                _copied ? 'скопировано' : 'ссылка',
                style: T.label(_copied ? C.signal : p.mute),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _Heart extends StatelessWidget {
  const _Heart({required this.liked, required this.onTap});
  final bool liked;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    return GestureDetector(
      onTap: onTap,
      child: AnimatedContainer(
        duration: const Duration(milliseconds: 160),
        width: 36,
        height: 36,
        decoration: BoxDecoration(
          color: liked ? C.signal.withValues(alpha: 0.15) : Colors.transparent,
          border: Border.all(color: liked ? C.signal : p.hairline),
          shape: BoxShape.circle,
        ),
        child: Icon(
          liked ? Icons.favorite_rounded : Icons.favorite_border_rounded,
          size: 17,
          color: liked ? C.signal : p.mute,
        ),
      ),
    );
  }
}

class _Back extends StatefulWidget {
  const _Back({required this.onTap});
  final VoidCallback onTap;

  @override
  State<_Back> createState() => _BackState();
}

class _BackState extends State<_Back> {
  bool _hover = false;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    return MouseRegion(
      cursor: SystemMouseCursors.click,
      onEnter: (_) => setState(() => _hover = true),
      onExit: (_) => setState(() => _hover = false),
      child: GestureDetector(
        behavior: HitTestBehavior.opaque,
        onTap: widget.onTap,
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 120),
          width: 38,
          height: 38,
          decoration: BoxDecoration(
            color: _hover ? p.card : Colors.transparent,
            shape: BoxShape.circle,
          ),
          child: Icon(Icons.arrow_back_rounded, size: 20, color: p.ink),
        ),
      ),
    );
  }
}
