import 'package:flutter/material.dart';

import '../api/client.dart';
import '../design/components.dart';
import '../design/tokens.dart';
import '../player/controller.dart';
import '../widgets/track_row.dart';

/// Opens when a mix or an album is pressed: the list first, the play button
/// second. Pressing a collection should never start audio unannounced.
/// A model of the collection the shell is showing. Navigation stays inside the
/// shell so the player bar and the tabs never disappear under a route.
class Collection {
  const Collection({
    required this.title,
    required this.subtitle,
    required this.tracks,
    this.coverUrl,
    this.mosaic = const [],
  });

  final String title;
  final String subtitle;
  final List<Track> tracks;
  final String? coverUrl;
  final List<String?> mosaic;
}

class CollectionView extends StatelessWidget {
  const CollectionView({
    super.key,
    required this.collection,
    required this.api,
    required this.player,
    required this.onRadio,
    required this.onBack,
    this.accent = C.signal,
  });

  final Collection collection;
  final Api api;
  final PlayerController player;
  final void Function(Track) onRadio;
  final VoidCallback onBack;
  final Color accent;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    final title = collection.title;
    final subtitle = collection.subtitle;
    final tracks = collection.tracks;
    final mosaic = collection.mosaic;
    final coverUrl = collection.coverUrl;
    final total = tracks.fold<Duration>(
      Duration.zero,
      (sum, t) => sum + t.duration,
    );
    final minutes = (total.inSeconds / 60).round();

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(S.lg, S.sm, S.lg, S.sm),
          child: Row(
            children: [
              _BackButton(onTap: onBack),
              const Spacer(),
            ],
          ),
        ),
        Hr(color: p.hairline),
        Expanded(
          child: ListView(
            padding: const EdgeInsets.only(bottom: S.xxl),
            children: [
              Padding(
                padding: const EdgeInsets.fromLTRB(S.xl, 0, S.xl, S.lg),
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.end,
                  children: [
                    if (mosaic.length >= 3)
                      _Mosaic(covers: mosaic, size: 168)
                    else if (coverUrl != null)
                      CoverArt(url: coverUrl, size: 168, radius: 12)
                    else
                      Hatch(size: 168, radius: 12),
                    const SizedBox(width: S.lg),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          Text(title, style: T.displayM(p.ink), maxLines: 2),
                          const SizedBox(height: S.xs),
                          Text(subtitle, style: T.caption(p.mute)),
                          const SizedBox(height: S.md),
                          Row(
                            children: [
                              PillButton(
                                icon: Icons.play_arrow_rounded,
                                label: 'play',
                                accent: accent,
                                onTap: tracks.isEmpty
                                    ? () {}
                                    : () => player.play(
                                        tracks.first,
                                        queue: tracks,
                                      ),
                              ),
                              const SizedBox(width: S.md),
                              Text(
                                '${tracks.length} ${plural(tracks.length)} · $minutes мин',
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
              Hr(color: p.hairlineStrong),
              for (var i = 0; i < tracks.length; i++)
                TrackRow(
                  key: ValueKey('col-${tracks[i].id}'),
                  track: tracks[i],
                  index: i + 1,
                  playing: player.current?.id == tracks[i].id,
                  coverUrl: api.cover(tracks[i].albumId, tracks[i].hasCover),
                  onTap: () => player.play(tracks[i], queue: tracks),
                  onRadio: () => onRadio(tracks[i]),
                ),
            ],
          ),
        ),
      ],
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
      borderRadius: BorderRadius.circular(12),
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

class _BackButton extends StatelessWidget {
  const _BackButton({required this.onTap});
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    return GestureDetector(
      behavior: HitTestBehavior.opaque,
      onTap: onTap,
      child: Row(
        children: [
          Icon(Icons.arrow_back_rounded, size: 20, color: p.ink),
          const SizedBox(width: 6),
          Text('назад', style: T.label(p.ink)),
        ],
      ),
    );
  }
}
