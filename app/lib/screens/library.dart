import 'package:flutter/material.dart';

import '../api/client.dart';
import '../design/components.dart';
import '../design/tokens.dart';
import '../player/controller.dart';
import '../player/likes.dart';
import '../widgets/track_menu.dart';
import '../widgets/track_row.dart';

/// The library as a real index: artists or albums, sorted, with an alphabet
/// rail that jumps instead of scrolling. The three dots on the rail are the
/// letters the library actually has, not the whole alphabet — an index that
/// jumps to nothing is worse than no index.
class LibraryScreen extends StatefulWidget {
  const LibraryScreen({
    super.key,
    required this.api,
    required this.player,
    required this.likes,
    required this.tracks,
    required this.onArtist,
    required this.onAlbum,
    required this.onRadio,
  });

  final Api api;
  final PlayerController player;
  final LikesStore likes;
  final List<Track> tracks;
  final void Function(int artistId) onArtist;
  final void Function(int albumId) onAlbum;
  final void Function(Track) onRadio;

  @override
  State<LibraryScreen> createState() => _LibraryScreenState();
}

class _LibraryScreenState extends State<LibraryScreen> {
  final ScrollController _scroll = ScrollController();
  final TextEditingController _query = TextEditingController();
  final Map<String, GlobalKey> _keys = {};
  String _q = '';
  int _mode = 0; // 0 artists, 1 albums, 2 tracks
  List<ArtistSummary> _artists = const [];
  List<AlbumSummary> _albums = const [];
  bool _loading = true;

  @override
  void initState() {
    super.initState();
    _load();
  }

  @override
  void dispose() {
    _scroll.dispose();
    _query.dispose();
    super.dispose();
  }

  Future<void> _load() async {
    try {
      final results = await Future.wait([
        widget.api.artists(),
        widget.api.albums(),
      ]);
      if (!mounted) return;
      setState(() {
        _artists = results[0] as List<ArtistSummary>;
        _albums = results[1] as List<AlbumSummary>;
        _loading = false;
      });
    } catch (_) {
      if (mounted) setState(() => _loading = false);
    }
  }

  String _letter(String name) {
    if (name.isEmpty) return '#';
    final c = name.characters.first.toUpperCase();
    if (RegExp(r'[0-9]').hasMatch(c)) return '#';
    return c;
  }

  List<dynamic> get _items {
    final q = _q.trim().toLowerCase();
    if (_mode == 0) {
      return _artists
          .where((a) => q.isEmpty || a.name.toLowerCase().contains(q))
          .toList(growable: false);
    }
    if (_mode == 1) {
      return _albums
          .where(
            (a) =>
                q.isEmpty ||
                a.title.toLowerCase().contains(q) ||
                a.artist.toLowerCase().contains(q),
          )
          .toList(growable: false);
    }
    return widget.tracks
        .where(
          (t) =>
              q.isEmpty ||
              t.title.toLowerCase().contains(q) ||
              t.artist.toLowerCase().contains(q) ||
              t.album.toLowerCase().contains(q),
        )
        .toList(growable: false);
  }

  String _itemLabel(dynamic item) => switch (item) {
    ArtistSummary a => a.name,
    AlbumSummary a => a.title,
    Track t => t.title,
    _ => '',
  };

  void _jumpTo(String letter) {
    final key = _keys[letter];
    final ctx = key?.currentContext;
    if (ctx != null) {
      Scrollable.ensureVisible(
        ctx,
        duration: const Duration(milliseconds: 320),
        curve: Curves.easeOutCubic,
        alignment: 0,
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    final items = _items;

    // Group headers only where the first letter changes.
    final groups = <String, int>{};
    for (var i = 0; i < items.length; i++) {
      final l = _letter(_itemLabel(items[i]));
      groups.putIfAbsent(l, () => i);
    }
    _keys
      ..clear()
      ..addEntries(groups.keys.map((l) => MapEntry(l, GlobalKey())));

    return Row(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Padding(
                padding: const EdgeInsets.fromLTRB(S.xl, S.xxl, S.xl, S.md),
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.end,
                  children: [
                    Text('библиотека', style: T.displayM(p.ink)),
                    const SizedBox(width: S.sm),
                    Text('${items.length}', style: T.caption(p.ash)),
                    const Spacer(),
                    _Tabs(
                      value: _mode,
                      onChanged: (v) => setState(() => _mode = v),
                    ),
                  ],
                ),
              ),
              Padding(
                padding: const EdgeInsets.fromLTRB(S.xl, 0, S.xl, S.md),
                child: Container(
                  height: 42,
                  padding: const EdgeInsets.symmetric(horizontal: S.md),
                  decoration: BoxDecoration(
                    color: p.soft,
                    border: Border.all(color: p.hairline),
                    borderRadius: BorderRadius.circular(12),
                  ),
                  child: Row(
                    children: [
                      Icon(Icons.search_rounded, size: 18, color: p.mute),
                      const SizedBox(width: S.sm),
                      Expanded(
                        child: TextField(
                          controller: _query,
                          onChanged: (v) => setState(() => _q = v),
                          cursorColor: C.signal,
                          style: T.body(p.ink),
                          decoration: InputDecoration(
                            isDense: true,
                            border: InputBorder.none,
                            hintText: _mode == 0
                                ? 'артист'
                                : _mode == 1
                                ? 'альбом или артист'
                                : 'трек, артист, альбом',
                            hintStyle: T.body(p.ash),
                          ),
                        ),
                      ),
                      if (_q.isNotEmpty)
                        IconButton(
                          icon: Icon(
                            Icons.close_rounded,
                            size: 16,
                            color: p.ash,
                          ),
                          onPressed: () {
                            _query.clear();
                            setState(() => _q = '');
                          },
                        ),
                    ],
                  ),
                ),
              ),
              Hr(color: p.hairlineStrong),
              Expanded(
                child: _loading
                    ? Center(
                        child: Text(
                          'читаю библиотеку…',
                          style: T.label(p.mute),
                        ),
                      )
                    : items.isEmpty
                    ? Center(
                        child: Text(
                          'ничего не найдено',
                          style: T.label(p.mute),
                        ),
                      )
                    : ListView.builder(
                        controller: _scroll,
                        padding: const EdgeInsets.only(bottom: S.xxl),
                        itemCount: items.length,
                        itemBuilder: (context, i) {
                          final item = items[i];
                          final label = _itemLabel(item);
                          final letter = _letter(label);
                          final header = groups[letter] == i;
                          return Column(
                            crossAxisAlignment: CrossAxisAlignment.stretch,
                            children: [
                              if (header)
                                Container(
                                  key: _keys[letter],
                                  color: p.canvas,
                                  padding: const EdgeInsets.fromLTRB(
                                    S.xl,
                                    S.md,
                                    S.xl,
                                    4,
                                  ),
                                  child: Text(
                                    letter,
                                    style: T.heading(C.signal),
                                  ),
                                ),
                              _row(context, item, i),
                            ],
                          );
                        },
                      ),
              ),
            ],
          ),
        ),
        // Alphabet rail.
        if (_mode != 2 && groups.isNotEmpty)
          Padding(
            padding: const EdgeInsets.fromLTRB(0, S.xl, S.sm, S.xl),
            child: Column(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                for (final letter in groups.keys)
                  GestureDetector(
                    onTap: () => _jumpTo(letter),
                    child: MouseRegion(
                      cursor: SystemMouseCursors.click,
                      child: Padding(
                        padding: const EdgeInsets.symmetric(vertical: 1.5),
                        child: Text(letter, style: T.micro(p.mute)),
                      ),
                    ),
                  ),
              ],
            ),
          ),
      ],
    );
  }

  Widget _row(BuildContext context, dynamic item, int i) {
    final p = Palette.of(context);
    if (item is ArtistSummary) {
      return _IndexRow(
        key: ValueKey('artist-${item.id}'),
        coverUrl: item.coverAlbum == null
            ? null
            : widget.api.cover(item.coverAlbum!, item.hasCover),
        title: item.name,
        subtitle:
            '${item.albums} ${plural(item.albums, one: 'альбом', few: 'альбома', many: 'альбомов')} · '
            '${item.tracks} ${plural(item.tracks, one: 'трек', few: 'трека', many: 'треков')}',
        badge: item.liked ? Icons.favorite_rounded : null,
        onTap: () => widget.onArtist(item.id),
      );
    }
    if (item is AlbumSummary) {
      return _IndexRow(
        key: ValueKey('album-${item.id}'),
        coverUrl: widget.api.cover(item.id, item.hasCover),
        title: item.title,
        subtitle: '${item.artist} · ${item.year ?? '—'}',
        badge: item.liked ? Icons.favorite_rounded : null,
        onTap: () => widget.onAlbum(item.id),
      );
    }
    final t = item as Track;
    return TrackRow(
      key: ValueKey('lib-${t.id}-$i'),
      track: t,
      playing: widget.player.current?.id == t.id,
      coverUrl: widget.api.cover(t.albumId, t.hasCover),
      index: i + 1,
      onTap: () => widget.player.play(t, queue: widget.tracks),
      onRadio: () => widget.onRadio(t),
      onMenu: () => showTrackMenu(
        context,
        track: t,
        api: widget.api,
        player: widget.player,
        likes: widget.likes,
        onRadio: () => widget.onRadio(t),
      ),
      liked: widget.likes.contains(t.id),
      onLike: () => widget.likes.toggle(t),
    );
  }
}

class _Tabs extends StatelessWidget {
  const _Tabs({required this.value, required this.onChanged});
  final int value;
  final ValueChanged<int> onChanged;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    const labels = ['артисты', 'альбомы', 'треки'];
    return Container(
      padding: const EdgeInsets.all(3),
      decoration: BoxDecoration(
        color: p.soft,
        border: Border.all(color: p.hairline),
        borderRadius: BorderRadius.circular(10),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          for (var i = 0; i < labels.length; i++)
            GestureDetector(
              onTap: () => onChanged(i),
              child: AnimatedContainer(
                duration: const Duration(milliseconds: 160),
                padding: const EdgeInsets.symmetric(
                  horizontal: 12,
                  vertical: 6,
                ),
                decoration: BoxDecoration(
                  color: value == i ? p.card : Colors.transparent,
                  borderRadius: BorderRadius.circular(8),
                ),
                child: Text(
                  labels[i],
                  style: T.label(value == i ? p.ink : p.mute),
                ),
              ),
            ),
        ],
      ),
    );
  }
}

class _IndexRow extends StatefulWidget {
  const _IndexRow({
    super.key,
    required this.title,
    required this.subtitle,
    required this.onTap,
    this.coverUrl,
    this.badge,
  });

  final String title;
  final String subtitle;
  final String? coverUrl;
  final IconData? badge;
  final VoidCallback onTap;

  @override
  State<_IndexRow> createState() => _IndexRowState();
}

class _IndexRowState extends State<_IndexRow> {
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
          padding: const EdgeInsets.symmetric(horizontal: S.xl, vertical: 7),
          child: Row(
            children: [
              widget.coverUrl == null
                  ? Container(
                      width: 44,
                      height: 44,
                      decoration: BoxDecoration(
                        color: p.soft,
                        borderRadius: BorderRadius.circular(10),
                      ),
                      child: Icon(
                        Icons.person_outline_rounded,
                        size: 20,
                        color: p.ash,
                      ),
                    )
                  : CoverArt(url: widget.coverUrl, size: 44, radius: 10),
              const SizedBox(width: S.md),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Text(
                      widget.title,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: T.bodyStrong(p.ink),
                    ),
                    Text(
                      widget.subtitle,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: T.caption(p.mute),
                    ),
                  ],
                ),
              ),
              if (widget.badge != null)
                Icon(widget.badge, size: 15, color: C.signal),
              const SizedBox(width: S.md),
              Icon(Icons.chevron_right_rounded, size: 18, color: p.ash),
            ],
          ),
        ),
      ),
    );
  }
}
