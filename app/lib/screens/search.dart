import 'dart:async';

import 'package:flutter/material.dart';

import '../api/client.dart';
import '../design/components.dart';
import '../design/tokens.dart';
import '../player/controller.dart';
import '../widgets/track_row.dart';

/// Global search: one field, results grouped the way people ask for them —
/// tracks first, then the albums the query touches.
class SearchScreen extends StatefulWidget {
  const SearchScreen({
    super.key,
    required this.api,
    required this.player,
    required this.onRadio,
  });

  final Api api;
  final PlayerController player;
  final void Function(Track) onRadio;

  @override
  State<SearchScreen> createState() => _SearchScreenState();
}

class _SearchScreenState extends State<SearchScreen> {
  final TextEditingController _query = TextEditingController();
  final FocusNode _focus = FocusNode();
  Timer? _debounce;
  List<Track> _results = const [];
  bool _loading = false;
  String _last = '';

  @override
  void initState() {
    super.initState();
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
        _results = const [];
        _last = '';
        _loading = false;
      });
      return;
    }
    setState(() => _loading = true);
    _debounce = Timer(const Duration(milliseconds: 220), () async {
      try {
        final results = await widget.api.search(value.trim());
        if (!mounted || _query.text.trim() != value.trim()) return;
        setState(() {
          _results = results;
          _last = value.trim();
          _loading = false;
        });
      } catch (_) {
        if (mounted) setState(() => _loading = false);
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    final albums = albumsOf(_results);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(S.xl, S.xxl, S.xl, S.md),
          child: Text('search', style: T.displayM(p.ink)),
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
                      hintText: 'tracks, artists, albums',
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
        Expanded(child: _body(context, p, albums)),
      ],
    );
  }

  Widget _body(BuildContext context, Palette p, List<Album> albums) {
    if (_last.isEmpty) {
      return Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(Icons.search_rounded, size: 34, color: p.ash),
            const SizedBox(height: S.md),
            Text('search the library', style: T.label(p.mute)),
            const SizedBox(height: S.xs),
            Text('artist, album, track name', style: T.caption(p.ash)),
          ],
        ),
      );
    }
    if (_loading && _results.isEmpty) {
      return Center(child: Text('searching…', style: T.label(p.mute)));
    }
    if (_results.isEmpty) {
      return Center(
        child: Text('nothing found for "$_last"', style: T.label(p.mute)),
      );
    }
    return ListView(
      padding: const EdgeInsets.only(bottom: S.xxl),
      children: [
        if (albums.isNotEmpty)
          Padding(
            padding: const EdgeInsets.fromLTRB(S.xl, S.lg, S.xl, S.sm),
            child: Row(
              children: [
                Text('albums', style: T.heading(p.ink)),
                const SizedBox(width: S.sm),
                Text('${albums.length}', style: T.caption(p.ash)),
                const SizedBox(width: S.md),
                Expanded(child: Hr(color: p.hairline)),
              ],
            ),
          ),
        if (albums.isNotEmpty)
          SizedBox(
            height: 208,
            child: ListView.separated(
              scrollDirection: Axis.horizontal,
              padding: const EdgeInsets.fromLTRB(S.xl, 0, S.xl, S.sm),
              itemCount: albums.length,
              separatorBuilder: (_, _) => const SizedBox(width: S.md),
              itemBuilder: (context, i) {
                final a = albums[i];
                return SizedBox(
                  width: 150,
                  child: GestureDetector(
                    onTap: () =>
                        widget.player.play(a.tracks.first, queue: a.tracks),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        CoverArt(
                          url: widget.api.cover(a.id, a.hasCover),
                          size: 150,
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
                          a.artist,
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
        Padding(
          padding: const EdgeInsets.fromLTRB(S.xl, S.lg, S.xl, S.sm),
          child: Row(
            children: [
              Text('tracks', style: T.heading(p.ink)),
              const SizedBox(width: S.sm),
              Text('${_results.length}', style: T.caption(p.ash)),
              const SizedBox(width: S.md),
              Expanded(child: Hr(color: p.hairline)),
            ],
          ),
        ),
        for (var i = 0; i < _results.length; i++)
          TrackRow(
            key: ValueKey('search-${_results[i].id}'),
            track: _results[i],
            index: i + 1,
            playing: widget.player.current?.id == _results[i].id,
            coverUrl: widget.api.cover(
              _results[i].albumId,
              _results[i].hasCover,
            ),
            onTap: () => widget.player.play(_results[i], queue: _results),
            onRadio: () => widget.onRadio(_results[i]),
          ),
      ],
    );
  }
}
