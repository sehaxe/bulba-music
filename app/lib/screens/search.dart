import 'dart:io';
import 'dart:async';

import 'package:flutter/material.dart';

import '../api/client.dart';
import '../design/components.dart';
import '../design/tokens.dart';
import '../player/controller.dart';
import '../player/likes.dart';
import '../widgets/track_menu.dart';
import '../widgets/track_row.dart';
import 'collection.dart';

/// Global search: one field, three sections. Pressing an artist narrows the
/// query to them instead of leaving the user to retype the name.
class SearchScreen extends StatefulWidget {
  const SearchScreen({
    super.key,
    required this.api,
    required this.player,
    required this.onRadio,
    required this.onOpen,
    required this.likes,
  });

  final Api api;
  final PlayerController player;
  final void Function(Track) onRadio;
  final void Function(Collection) onOpen;
  final LikesStore likes;

  @override
  State<SearchScreen> createState() => _SearchScreenState();
}

class _SearchScreenState extends State<SearchScreen> {
  final TextEditingController _query = TextEditingController();
  final FocusNode _focus = FocusNode();
  Timer? _debounce;
  SearchResults? _results;
  MoodResult? _mood;
  bool _loading = false;

  @override
  void initState() {
    super.initState();
    // `BULBA_SEARCH=чиптюн` types a query on launch: the mood block only shows
    // when a sentence was understood, and that is worth a screenshot.
    final q = Platform.environment['BULBA_SEARCH'];
    if (q != null && q.isNotEmpty) {
      WidgetsBinding.instance.addPostFrameCallback((_) => _searchFor(q));
    }
    WidgetsBinding.instance.addPostFrameCallback((_) => _focus.requestFocus());
  }

  @override
  void dispose() {
    _debounce?.cancel();
    _query.dispose();
    _focus.dispose();
    super.dispose();
  }

  void _onChanged(String value) {
    _debounce?.cancel();
    if (value.trim().isEmpty) {
      setState(() {
        _results = null;
        _loading = false;
      });
      return;
    }
    setState(() {
      _loading = true;
      _mood = null;
    });
    _debounce = Timer(const Duration(milliseconds: 220), () async {
      // Two questions at once: does a title match, and does the sentence mean
      // anything? They are cheap and the second one is the interesting one.
      final moodFuture = widget.api
          .mood(value.trim(), limit: 12)
          .catchError((_) => const MoodResult(phrase: null, tracks: []));
      try {
        final results = await widget.api.search(value.trim());
        final mood = await moodFuture;
        if (!mounted || _query.text.trim() != value.trim()) return;
        setState(() {
          _results = results;
          _mood = mood;
          _loading = false;
        });
      } catch (_) {
        if (mounted) setState(() => _loading = false);
      }
    });
  }

  Widget _moodSection(MoodResult mood) {
    final p = Palette.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(S.xl, S.lg, S.xl, S.xs),
          child: Row(
            children: [
              Icon(Icons.graphic_eq_rounded, size: 16, color: p.mute),
              const SizedBox(width: S.sm),
              Text('по звуку: «${mood.phrase}»', style: T.label(p.mute)),
            ],
          ),
        ),
        for (final t in mood.tracks)
          TrackRow(
            track: t,
            coverUrl: widget.api.cover(t.albumId, t.hasCover, w: 128),
            showCover: true,
            playing: widget.player.current?.id == t.id,
            liked: widget.likes.contains(t.id),
            onLike: () => widget.likes.toggle(t),
            onTap: () => widget.player.play(t, queue: mood.tracks),
            onRadio: () => widget.onRadio(t),
            onMenu: () => showTrackMenu(
              context,
              track: t,
              api: widget.api,
              player: widget.player,
              likes: widget.likes,
              onRadio: () => widget.onRadio(t),
            ),
          ),
        Hr(color: p.hairline),
      ],
    );
  }

  void _searchFor(String text) {
    _query.text = text;
    _query.selection = TextSelection.collapsed(offset: text.length);
    _onChanged(text);
  }

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(S.xl, S.xxl, S.xl, S.md),
          child: Text('поиск', style: T.displayM(p.ink)),
        ),
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: S.xl),
          child: Container(
            height: 44,
            padding: const EdgeInsets.symmetric(horizontal: S.md),
            decoration: BoxDecoration(
              color: p.soft,
              border: Border.all(color: p.hairline),
              borderRadius: BorderRadius.circular(12),
            ),
            child: Row(
              children: [
                Icon(Icons.search_rounded, size: 20, color: p.mute),
                const SizedBox(width: S.sm),
                Expanded(
                  child: TextField(
                    controller: _query,
                    focusNode: _focus,
                    onChanged: _onChanged,
                    cursorColor: C.signal,
                    style: T.body(p.ink),
                    decoration: InputDecoration(
                      isDense: true,
                      border: InputBorder.none,
                      hintText: 'трек, артист, альбом — или обрывок',
                      hintStyle: T.body(p.ash),
                    ),
                  ),
                ),
                if (_query.text.isNotEmpty)
                  IconButton(
                    icon: Icon(Icons.close_rounded, size: 18, color: p.mute),
                    onPressed: () {
                      _query.clear();
                      _onChanged('');
                      _focus.requestFocus();
                    },
                  ),
              ],
            ),
          ),
        ),
        const SizedBox(height: S.md),
        Hr(color: p.hairlineStrong),
        Expanded(child: _body(p)),
      ],
    );
  }

  Widget _body(Palette p) {
    final results = _results;
    if (_query.text.trim().isEmpty) {
      return Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(Icons.search_rounded, size: 34, color: p.ash),
            const SizedBox(height: S.md),
            Text('поиск по библиотеке', style: T.label(p.mute)),
            const SizedBox(height: S.xs),
            Text(
              'обрывок работает: «happ» находит «Let It Happen»',
              style: T.caption(p.ash),
            ),
          ],
        ),
      );
    }
    if (_loading && results == null) {
      return Center(child: Text('ищу…', style: T.label(p.mute)));
    }
    if (results == null || results.isEmpty) {
      // Text search failing does not mean the library has nothing: "грустный
      // джаз на ночь" is a description, not a title.
      return ListView(
        padding: const EdgeInsets.only(bottom: S.xxl),
        children: [
          Center(
            child: Text(
              'nothing by name for "${_query.text.trim()}"',
              style: T.label(p.mute),
            ),
          ),
          if (_mood?.tracks.isNotEmpty ?? false) _moodSection(_mood!),
        ],
      );
    }

    return ListView(
      padding: const EdgeInsets.only(bottom: S.xxl),
      children: [
        if (_mood?.tracks.isNotEmpty ?? false) _moodSection(_mood!),
        if (results.artists.isNotEmpty) ...[
          _SectionHeader(label: 'артисты', count: results.artists.length),
          Padding(
            padding: const EdgeInsets.fromLTRB(S.xl, 0, S.xl, S.md),
            child: Wrap(
              spacing: S.sm,
              runSpacing: S.sm,
              children: [
                for (final a in results.artists)
                  _ArtistChip(artist: a, onTap: () => _searchFor(a.name)),
              ],
            ),
          ),
        ],
        if (results.albums.isNotEmpty) ...[
          _SectionHeader(label: 'альбомы', count: results.albums.length),
          SizedBox(
            height: 206,
            child: ListView.separated(
              scrollDirection: Axis.horizontal,
              padding: const EdgeInsets.fromLTRB(S.xl, 0, S.xl, S.md),
              itemCount: results.albums.length,
              separatorBuilder: (_, _) => const SizedBox(width: S.md),
              itemBuilder: (context, i) {
                final a = results.albums[i];
                return SizedBox(
                  width: 148,
                  child: GestureDetector(
                    onTap: () async {
                      // Fetch the album's tracks through search so the album
                      // page opens with a real queue.
                      final hit = await widget.api.search(a.title);
                      if (!context.mounted) return;
                      final tracks = hit.tracks
                          .where((t) => t.albumId == a.id)
                          .toList(growable: false);
                      widget.onOpen(
                        Collection(
                          title: a.title,
                          subtitle: [
                            a.artist,
                            if (a.year != null) '${a.year}',
                          ].where((e) => e.isNotEmpty).join(' · '),
                          tracks: tracks,
                          coverUrl: widget.api.cover(a.id, a.hasCover, w: 128),
                        ),
                      );
                    },
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        CoverArt(
                          url: widget.api.cover(a.id, a.hasCover, w: 320),
                          size: 148,
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
                          '${a.artist} · ${a.tracks} ${plural(a.tracks)}',
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: T.micro(p.mute),
                        ),
                      ],
                    ),
                  ),
                );
              },
            ),
          ),
        ],
        if (results.tracks.isNotEmpty) ...[
          _SectionHeader(label: 'треки', count: results.tracks.length),
          for (var i = 0; i < results.tracks.length; i++)
            TrackRow(
              key: ValueKey('search-${results.tracks[i].id}'),
              track: results.tracks[i],
              index: i + 1,
              playing: widget.player.current?.id == results.tracks[i].id,
              coverUrl: widget.api.cover(
                results.tracks[i].albumId,
                results.tracks[i].hasCover,
              ),
              onTap: () =>
                  widget.player.play(results.tracks[i], queue: results.tracks),
              onRadio: () => widget.onRadio(results.tracks[i]),
              onMenu: () => showTrackMenu(
                context,
                track: results.tracks[i],
                api: widget.api,
                player: widget.player,
                likes: widget.likes,
                onRadio: () => widget.onRadio(results.tracks[i]),
              ),
              liked: widget.likes.contains(results.tracks[i].id),
              onLike: () => widget.likes.toggle(results.tracks[i]),
            ),
        ],
      ],
    );
  }
}

class _SectionHeader extends StatelessWidget {
  const _SectionHeader({required this.label, required this.count});
  final String label;
  final int count;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    return Padding(
      padding: const EdgeInsets.fromLTRB(S.xl, S.lg, S.xl, S.md),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.end,
        children: [
          Text(label, style: T.heading(p.ink)),
          const SizedBox(width: S.sm),
          Text('$count', style: T.caption(p.ash)),
          const SizedBox(width: S.md),
          Expanded(
            child: Padding(
              padding: const EdgeInsets.only(bottom: 3),
              child: Hr(color: p.hairline),
            ),
          ),
        ],
      ),
    );
  }
}

class _ArtistChip extends StatefulWidget {
  const _ArtistChip({required this.artist, required this.onTap});
  final SearchArtist artist;
  final VoidCallback onTap;

  @override
  State<_ArtistChip> createState() => _ArtistChipState();
}

class _ArtistChipState extends State<_ArtistChip> {
  bool _hover = false;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    return MouseRegion(
      cursor: SystemMouseCursors.click,
      onEnter: (_) => setState(() => _hover = true),
      onExit: (_) => setState(() => _hover = false),
      child: GestureDetector(
        onTap: widget.onTap,
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 140),
          padding: const EdgeInsets.symmetric(horizontal: S.md, vertical: 8),
          decoration: BoxDecoration(
            color: _hover ? p.card : p.soft,
            border: Border.all(color: p.hairline),
            borderRadius: BorderRadius.circular(10),
          ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(Icons.person_outline_rounded, size: 15, color: p.mute),
              const SizedBox(width: 6),
              Text(widget.artist.name, style: T.label(p.ink)),
              const SizedBox(width: 6),
              Text('${widget.artist.tracks}', style: T.micro(p.ash)),
            ],
          ),
        ),
      ),
    );
  }
}
