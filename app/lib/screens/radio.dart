import 'package:flutter/material.dart';

import '../api/client.dart';
import '../design/components.dart';
import '../design/tokens.dart';
import '../player/controller.dart';
import '../widgets/track_row.dart';

/// The queue, shown as a radio: the seed and the tracks the recommender chose.
class RadioScreen extends StatelessWidget {
  const RadioScreen({
    super.key,
    required this.player,
    required this.api,
    required this.onRadio,
  });

  final PlayerController player;
  final Api api;
  final void Function(Track) onRadio;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    final queue = player.queue;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(S.xl, S.xxl, S.xl, 0),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.end,
            children: [
              Text('radio', style: T.heading(p.ink)),
              const SizedBox(width: S.sm),
              Text('${queue.length}', style: T.caption(p.ash)),
              const Spacer(),
              Text('right-click a track → radio', style: T.micro(p.ash)),
            ],
          ),
        ),
        const SizedBox(height: S.md),
        Hr(color: p.hairlineStrong),
        Expanded(
          child: queue.isEmpty
              ? Center(
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Text('no radio yet', style: T.displayM(p.mute)),
                      const SizedBox(height: S.sm),
                      Text(
                        'long-press or right-click any track',
                        style: T.caption(p.ash),
                      ),
                    ],
                  ),
                )
              : ListView.builder(
                  padding: const EdgeInsets.only(bottom: S.xxl),
                  itemCount: queue.length,
                  itemBuilder: (context, i) {
                    final t = queue[i];
                    return TrackRow(
                      track: t,
                      index: i + 1,
                      playing: player.current?.id == t.id,
                      coverUrl: api.cover(t.albumId, t.hasCover),
                      onTap: () => player.play(t, queue: queue),
                      onRadio: () => onRadio(t),
                    );
                  },
                ),
        ),
      ],
    );
  }
}
