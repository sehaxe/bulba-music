import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../api/client.dart';
import '../design/components.dart';
import '../design/tokens.dart';
import '../player/controller.dart';
import '../player/likes.dart';

/// An artist is a page, not a filter: their albums in release order, what you
/// actually replay, and a heart that means "I follow this".
class ArtistScreen extends StatefulWidget {
  const ArtistScreen({
    super.key,
    required this.api,
    required this.player,
    required this.likes,
    required this.artistId,
    required this.onAlbum,
    required this.onRadio,
    required this.onBack,
  });

  final Api api;
  final PlayerController player;
  final LikesStore likes;
  final int artistId;
  final void Function(int albumId) onAlbum;
  final void Function(Track) onRadio;
  final VoidCallback onBack;

  @override
  State<ArtistScreen> createState() => _ArtistScreenState();
}

class _ArtistScreenState extends State<ArtistScreen> {
  ArtistPage? _page;
  bool _liked = false;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    try {
      final page = await widget.api.artistPage(widget.artistId);
      if (!mounted) return;
      setState(() {
        _page = page;
        _liked = page.artist.liked;
      });
    } catch (_) {}
  }

  Future<void> _toggleLike() async {
    final next = !_liked;
    setState(() => _liked = next);
    try {
      await widget.api.likeArtist(widget.artistId, on: next);
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
    final artist = page.artist;
    final years = [
      if (page.firstYear != null) '${page.firstYear}',
      if (page.lastYear != null && page.lastYear != page.firstYear)
        '${page.lastYear}',
    ].join('–');

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
                    _ArtistArt(api: widget.api, artist: artist),
                    const SizedBox(width: S.lg),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          Text('артист', style: T.micro(p.ash)),
                          const SizedBox(height: 2),
                          Text(artist.name, style: T.displayL(p.ink)),
                          const SizedBox(height: S.xs),
                          Text(
                            [
                              '${artist.albums} ${plural(artist.albums, one: 'альбом', few: 'альбома', many: 'альбомов')}',
                              '${artist.tracks} ${plural(artist.tracks, one: 'трек', few: 'трека', many: 'треков')}',
                              if (years.isNotEmpty) years,
                              if (artist.plays > 0) '${artist.plays} включений',
                            ].join(' · '),
                            style: T.caption(p.mute),
                          ),
                          const SizedBox(height: S.md),
                          Row(
                            children: [
                              PillButton(
                                icon: Icons.play_arrow_rounded,
                                label: 'play',
                                onTap: page.topTracks.isEmpty
                                    ? null
                                    : () async {
                                        final first = page.topTracks.first;
                                        final track = await widget.api.track(
                                          first.id,
                                        );
                                        if (track == null) return;
                                        final queue = <Track>[];
                                        for (final t in page.topTracks) {
                                          final full = await widget.api.track(
                                            t.id,
                                          );
                                          if (full != null) queue.add(full);
                                        }
                                        widget.player.play(track, queue: queue);
                                      },
                              ),
                              const SizedBox(width: S.sm),
                              _Heart(liked: _liked, onTap: _toggleLike),
                            ],
                          ),
                        ],
                      ),
                    ),
                  ],
                ),
              ),
              Hr(color: p.hairlineStrong),
              if (page.topTracks.isNotEmpty) ...[
                Padding(
                  padding: const EdgeInsets.fromLTRB(S.xl, S.lg, S.xl, S.sm),
                  child: Row(
                    children: [
                      Text('часто слушаешь', style: T.heading(p.ink)),
                      const SizedBox(width: S.md),
                      Expanded(child: Hr(color: p.hairline)),
                    ],
                  ),
                ),
                for (var i = 0; i < page.topTracks.length; i++)
                  _TopRow(
                    index: i + 1,
                    track: page.topTracks[i],
                    coverUrl: widget.api.cover(
                      page.topTracks[i].albumId,
                      page.topTracks[i].hasCover,
                    ),
                    onTap: () async {
                      final t = await widget.api.track(page.topTracks[i].id);
                      if (t == null || !mounted) return;
                      final queue = <Track>[];
                      for (final e in page.topTracks) {
                        final full = await widget.api.track(e.id);
                        if (full != null) queue.add(full);
                      }
                      widget.player.play(t, queue: queue);
                    },
                  ),
              ],
              Padding(
                padding: const EdgeInsets.fromLTRB(S.xl, S.xl, S.xl, S.sm),
                child: Row(
                  children: [
                    Text('альбомы', style: T.heading(p.ink)),
                    const SizedBox(width: S.md),
                    Expanded(child: Hr(color: p.hairline)),
                  ],
                ),
              ),
              Padding(
                padding: const EdgeInsets.fromLTRB(S.xl, 0, S.xl, 0),
                child: Wrap(
                  spacing: S.md,
                  runSpacing: S.lg,
                  children: [
                    for (final a in page.albums)
                      GestureDetector(
                        onTap: () => widget.onAlbum(a.id),
                        child: SizedBox(
                          width: 156,
                          child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            mainAxisSize: MainAxisSize.min,
                            children: [
                              CoverArt(
                                url: widget.api.cover(a.id, a.hasCover),
                                size: 156,
                                radius: 10,
                              ),
                              const SizedBox(height: S.sm),
                              Text(
                                a.title,
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                                style: T.bodyStrong(p.ink),
                              ),
                              Text(
                                '${a.year ?? '—'} · ${a.tracks} ${plural(a.tracks, one: 'трек', few: 'трека', many: 'треков')}',
                                style: T.micro(p.mute),
                              ),
                            ],
                          ),
                        ),
                      ),
                  ],
                ),
              ),
            ],
          ),
        ),
      ],
    );
  }
}

class _LinkButton extends StatefulWidget {
  const _LinkButton({required this.path, required this.base});
  final String path;
  final String base;

  @override
  State<_LinkButton> createState() => _LinkButtonState();
}

class _LinkButtonState extends State<_LinkButton> {
  bool _copied = false;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    return GestureDetector(
      onTap: () async {
        // The link points at the server, so it works from any device on the
        // same network (or Tailscale) without the app installed.
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

class _ArtistArt extends StatelessWidget {
  const _ArtistArt({required this.api, required this.artist});
  final Api api;
  final ArtistSummary artist;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    final url = artist.coverAlbum == null
        ? null
        : api.cover(artist.coverAlbum!, artist.hasCover);
    return Container(
      width: 148,
      height: 148,
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: p.hairline),
      ),
      clipBehavior: Clip.antiAlias,
      child: url == null
          ? Icon(Icons.person_rounded, size: 48, color: p.ash)
          : CoverArt(url: url, size: 148, radius: 0, border: false),
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

class _TopRow extends StatefulWidget {
  const _TopRow({
    required this.index,
    required this.track,
    required this.coverUrl,
    required this.onTap,
  });
  final int index;
  final StatTrack track;
  final String? coverUrl;
  final VoidCallback onTap;

  @override
  State<_TopRow> createState() => _TopRowState();
}

class _TopRowState extends State<_TopRow> {
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
          duration: const Duration(milliseconds: 90),
          color: _hover ? p.soft : Colors.transparent,
          padding: const EdgeInsets.symmetric(horizontal: S.xl, vertical: 6),
          child: Row(
            children: [
              SizedBox(
                width: 24,
                child: Text(
                  '${widget.index}',
                  textAlign: TextAlign.right,
                  style: T.caption(p.ash),
                ),
              ),
              const SizedBox(width: S.md),
              CoverArt(url: widget.coverUrl, size: 36),
              const SizedBox(width: S.md),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Text(
                      widget.track.title,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: T.bodyStrong(p.ink),
                    ),
                    Text(
                      widget.track.album,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: T.micro(p.mute),
                    ),
                  ],
                ),
              ),
              Text('${widget.track.plays}×', style: T.caption(p.mute)),
            ],
          ),
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
