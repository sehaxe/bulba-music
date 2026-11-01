import 'package:flutter/material.dart';

import '../api/client.dart';
import '../design/components.dart';
import '../design/tokens.dart';
import '../player/controller.dart';
import '../player/likes.dart';

/// The queue lives in a sheet, not on its own tab: it is a property of what is
/// playing, and opening it should never feel like leaving the player.
Future<void> showQueue(
  BuildContext context, {
  required PlayerController player,
  required Api api,
  required LikesStore likes,
}) {
  return showModalBottomSheet<void>(
    context: context,
    isScrollControlled: true,
    backgroundColor: Colors.transparent,
    barrierColor: Colors.black54,
    builder: (_) => _QueueSheet(player: player, api: api, likes: likes),
  );
}

class _QueueSheet extends StatefulWidget {
  const _QueueSheet({
    required this.player,
    required this.api,
    required this.likes,
  });
  final PlayerController player;
  final Api api;
  final LikesStore likes;

  @override
  State<_QueueSheet> createState() => _QueueSheetState();
}

class _QueueSheetState extends State<_QueueSheet> {
  @override
  void initState() {
    super.initState();
    widget.player.addListener(_rebuild);
    widget.likes.addListener(_rebuild);
  }

  @override
  void dispose() {
    widget.player.removeListener(_rebuild);
    widget.likes.removeListener(_rebuild);
    super.dispose();
  }

  void _rebuild() {
    if (mounted) setState(() {});
  }

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    final queue = widget.player.queue;
    final index = widget.player.queueIndex;
    final height = MediaQuery.sizeOf(context).height * 0.72;

    return Container(
      height: height,
      decoration: BoxDecoration(
        color: p.canvas,
        borderRadius: const BorderRadius.vertical(top: Radius.circular(18)),
        border: Border.all(color: p.hairline),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          // Drag handle: the sheet closes on a downward flick.
          GestureDetector(
            onVerticalDragEnd: (d) {
              if ((d.primaryVelocity ?? 0) > 200) Navigator.of(context).pop();
            },
            child: SizedBox(
              height: 26,
              child: Center(
                child: Container(
                  width: 36,
                  height: 4,
                  decoration: BoxDecoration(
                    color: p.hairlineStrong,
                    borderRadius: BorderRadius.circular(2),
                  ),
                ),
              ),
            ),
          ),
          Padding(
            padding: const EdgeInsets.fromLTRB(S.xl, 0, S.lg, S.md),
            child: Row(
              children: [
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text('queue', style: T.heading(p.ink)),
                      Text(
                        '${queue.length} · перетащи, чтобы поменять',
                        style: T.caption(p.mute),
                      ),
                    ],
                  ),
                ),
                IconButton(
                  icon: Icon(Icons.close_rounded, size: 20, color: p.mute),
                  onPressed: () => Navigator.of(context).pop(),
                ),
              ],
            ),
          ),
          Hr(color: p.hairlineStrong),
          Expanded(
            child: queue.isEmpty
                ? Center(child: Text('очередь пуста', style: T.label(p.mute)))
                : ReorderableListView.builder(
                    buildDefaultDragHandles: false,
                    padding: const EdgeInsets.only(bottom: S.lg),
                    itemCount: queue.length,
                    onReorderItem: (from, to) => widget.player.move(from, to),
                    itemBuilder: (context, i) {
                      final t = queue[i];
                      final playing = i == index;
                      return _QueueRow(
                        key: ValueKey('q-${t.id}-$i'),
                        index: i,
                        track: t,
                        playing: playing,
                        coverUrl: widget.api.cover(t.albumId, t.hasCover),
                        liked: widget.likes.contains(t.id),
                        onPlay: () => widget.player.jumpTo(i),
                        onLike: () => widget.likes.toggle(t),
                        onRemove: () => widget.player.removeAt(i),
                      );
                    },
                  ),
          ),
        ],
      ),
    );
  }
}

class _QueueRow extends StatelessWidget {
  const _QueueRow({
    super.key,
    required this.index,
    required this.track,
    required this.playing,
    required this.coverUrl,
    required this.liked,
    required this.onPlay,
    required this.onLike,
    required this.onRemove,
  });

  final int index;
  final Track track;
  final bool playing;
  final String? coverUrl;
  final bool liked;
  final VoidCallback onPlay;
  final VoidCallback onLike;
  final VoidCallback onRemove;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    return GestureDetector(
      behavior: HitTestBehavior.opaque,
      onTap: onPlay,
      child: Container(
        color: playing ? p.card : Colors.transparent,
        padding: const EdgeInsets.symmetric(horizontal: S.lg, vertical: 6),
        child: Row(
          children: [
            CoverArt(url: coverUrl, size: 34),
            const SizedBox(width: S.md),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                mainAxisSize: MainAxisSize.min,
                children: [
                  Text(
                    track.title,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: T.bodyStrong(
                      playing ? AccentScope.of(context) : p.ink,
                    ),
                  ),
                  Text(
                    track.artist,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: T.micro(p.mute),
                  ),
                ],
              ),
            ),
            IconButton(
              icon: Icon(
                liked ? Icons.favorite_rounded : Icons.favorite_border_rounded,
                size: 16,
                color: liked ? AccentScope.of(context) : p.ash,
              ),
              onPressed: onLike,
            ),
            IconButton(
              icon: Icon(Icons.close_rounded, size: 16, color: p.ash),
              onPressed: onRemove,
            ),
            ReorderableDragStartListener(
              index: index,
              child: MouseRegion(
                cursor: SystemMouseCursors.grab,
                child: Icon(
                  Icons.drag_indicator_rounded,
                  size: 18,
                  color: p.mute,
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
