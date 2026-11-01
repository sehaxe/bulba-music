import 'package:flutter/material.dart';

import '../api/client.dart';
import '../design/components.dart';
import '../design/tokens.dart';
import '../player/controller.dart';
import '../player/likes.dart';
import '../widgets/track_row.dart';

/// Everything the listener hearted, newest first. This list is the one thing a
/// recommender can never throw away.
class LikedScreen extends StatefulWidget {
  const LikedScreen({
    super.key,
    required this.api,
    required this.player,
    required this.likes,
    required this.onRadio,
    required this.onOpenCollection,
  });

  final Api api;
  final PlayerController player;
  final LikesStore likes;
  final void Function(Track) onRadio;
  final void Function(List<Track>, String) onOpenCollection;

  @override
  State<LikedScreen> createState() => _LikedScreenState();
}

class _LikedScreenState extends State<LikedScreen> {
  @override
  void initState() {
    super.initState();
    widget.likes.addListener(_rebuild);
    widget.likes.refreshTracks();
  }

  @override
  void dispose() {
    widget.likes.removeListener(_rebuild);
    super.dispose();
  }

  void _rebuild() {
    if (mounted) setState(() {});
  }

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    final tracks = widget.likes.tracks;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(S.xl, S.xxl, S.xl, S.sm),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.end,
            children: [
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text('любимое', style: T.displayM(p.ink)),
                    const SizedBox(height: S.xxs),
                    Text(
                      '${tracks.length} ${plural(tracks.length)}',
                      style: T.caption(p.mute),
                    ),
                  ],
                ),
              ),
              if (tracks.isNotEmpty)
                PillButton(
                  icon: Icons.play_arrow_rounded,
                  label: 'play',
                  onTap: () => widget.player.play(tracks.first, queue: tracks),
                ),
            ],
          ),
        ),
        const SizedBox(height: S.md),
        Hr(color: p.hairlineStrong),
        Expanded(
          child: widget.likes.loading
              ? Center(child: Text('loading…', style: T.label(p.mute)))
              : tracks.isEmpty
              ? Center(
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Icon(
                        Icons.favorite_border_rounded,
                        size: 34,
                        color: p.ash,
                      ),
                      const SizedBox(height: S.md),
                      Text('пока пусто', style: T.label(p.mute)),
                      const SizedBox(height: S.xs),
                      Text(
                        'нажми ♥ на треке или в плеере',
                        style: T.caption(p.ash),
                      ),
                    ],
                  ),
                )
              : ListView.builder(
                  padding: const EdgeInsets.only(bottom: S.xxl),
                  itemCount: tracks.length,
                  itemBuilder: (context, i) {
                    final t = tracks[i];
                    return TrackRow(
                      key: ValueKey('liked-${t.id}'),
                      track: t,
                      index: i + 1,
                      playing: widget.player.current?.id == t.id,
                      coverUrl: widget.api.cover(t.albumId, t.hasCover),
                      onTap: () => widget.player.play(t, queue: tracks),
                      onRadio: () => widget.onRadio(t),
                      liked: true,
                      onLike: () => widget.likes.toggle(t),
                    );
                  },
                ),
        ),
      ],
    );
  }
}
