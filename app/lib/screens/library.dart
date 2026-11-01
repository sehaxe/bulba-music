import 'package:flutter/material.dart';

import '../api/client.dart';
import '../design/components.dart';
import '../design/tokens.dart';
import '../player/controller.dart';
import '../widgets/track_row.dart';

class LibraryScreen extends StatefulWidget {
  const LibraryScreen({
    super.key,
    required this.tracks,
    required this.api,
    required this.player,
  });

  final List<Track> tracks;
  final Api api;
  final PlayerController player;

  @override
  State<LibraryScreen> createState() => _LibraryScreenState();
}

class _LibraryScreenState extends State<LibraryScreen> {
  final TextEditingController _query = TextEditingController();
  String _q = '';

  @override
  void dispose() {
    _query.dispose();
    super.dispose();
  }

  List<Track> get _filtered {
    final q = _q.trim().toLowerCase();
    if (q.isEmpty) return widget.tracks;
    return widget.tracks
        .where(
          (t) =>
              t.title.toLowerCase().contains(q) ||
              t.artist.toLowerCase().contains(q) ||
              t.album.toLowerCase().contains(q),
        )
        .toList(growable: false);
  }

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    final rows = _filtered;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(S.xl, S.xxl, S.xl, 0),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.end,
            children: [
              Text('library', style: T.heading(p.ink)),
              const SizedBox(width: S.sm),
              Text('${widget.tracks.length}', style: T.caption(p.ash)),
              const Spacer(),
              BracketButton(
                'shuffle',
                size: 12,
                color: p.mute,
                onTap: () {
                  if (widget.tracks.isNotEmpty) {
                    widget.player.play(
                      widget.tracks.first,
                      queue: widget.tracks,
                    );
                  }
                },
              ),
            ],
          ),
        ),
        Padding(
          padding: const EdgeInsets.fromLTRB(S.xl, S.md, S.xl, S.md),
          child: Container(
            height: 38,
            padding: const EdgeInsets.symmetric(horizontal: S.md),
            decoration: BoxDecoration(
              color: p.soft,
              border: Border.all(color: p.hairline),
              borderRadius: BorderRadius.circular(R.sm),
            ),
            child: Row(
              children: [
                Text('>', style: T.body(C.signal)),
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
                      hintText: 'search tracks, artists, albums',
                      hintStyle: T.body(p.ash),
                    ),
                  ),
                ),
                if (_q.isNotEmpty)
                  BracketButton(
                    'x',
                    size: 12,
                    color: p.ash,
                    onTap: () {
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
          child: rows.isEmpty
              ? Center(child: Text('[ nothing found ]', style: T.label(p.mute)))
              : ListView.builder(
                  padding: const EdgeInsets.only(bottom: S.xxl),
                  itemCount: rows.length,
                  itemBuilder: (context, i) {
                    final t = rows[i];
                    return StaggerIn(
                      key: ValueKey('lib-${t.id}-$i'),
                      index: i,
                      child: TrackRow(
                        track: t,
                        playing: widget.player.current?.id == t.id,
                        coverUrl: widget.api.cover(t.albumId, t.hasCover),
                        index: i + 1,
                        onTap: () => widget.player.play(t, queue: rows),
                      ),
                    );
                  },
                ),
        ),
      ],
    );
  }
}

class PlaceholderScreen extends StatelessWidget {
  const PlaceholderScreen({super.key, required this.title, required this.note});
  final String title;
  final String note;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Text('[$title]', style: T.displayM(p.mute)),
          const SizedBox(height: S.sm),
          Text(note, style: T.caption(p.ash)),
        ],
      ),
    );
  }
}
