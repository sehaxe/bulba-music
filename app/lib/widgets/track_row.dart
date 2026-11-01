import 'package:flutter/material.dart';

import '../api/client.dart';
import '../design/components.dart';
import '../design/tokens.dart';

class TrackRow extends StatefulWidget {
  const TrackRow({
    super.key,
    required this.track,
    required this.playing,
    required this.coverUrl,
    required this.onTap,
    this.showCover = true,
    this.index,
  });

  final Track track;
  final bool playing;
  final String? coverUrl;
  final VoidCallback onTap;
  final bool showCover;
  final int? index;

  @override
  State<TrackRow> createState() => _TrackRowState();
}

class _TrackRowState extends State<TrackRow> {
  bool _hover = false;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    final t = widget.track;
    return MouseRegion(
      cursor: SystemMouseCursors.click,
      onEnter: (_) => setState(() => _hover = true),
      onExit: (_) => setState(() => _hover = false),
      child: GestureDetector(
        behavior: HitTestBehavior.opaque,
        onTap: widget.onTap,
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 90),
          curve: Curves.easeOut,
          color: widget.playing
              ? p.card
              : _hover
              ? p.soft
              : Colors.transparent,
          padding: const EdgeInsets.symmetric(horizontal: S.xl, vertical: S.sm),
          child: Row(
            children: [
              if (widget.index != null) ...[
                SizedBox(
                  width: 26,
                  child: Text(
                    widget.playing ? '▸' : '${widget.index}',
                    textAlign: TextAlign.right,
                    style: T.caption(widget.playing ? C.signal : p.ash),
                  ),
                ),
                const SizedBox(width: S.md),
              ],
              if (widget.showCover) ...[
                CoverArt(url: widget.coverUrl, size: 40),
                const SizedBox(width: S.md),
              ],
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Text(
                      t.title,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: T.bodyStrong(widget.playing ? C.signal : p.ink),
                    ),
                    Text(
                      widget.showCover ? t.artist : '${t.artist} · ${t.album}',
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: T.caption(p.mute),
                    ),
                  ],
                ),
              ),
              const SizedBox(width: S.lg),
              SizedBox(
                width: 96,
                child: Text(
                  '[${t.format}]',
                  textAlign: TextAlign.right,
                  style: T.micro(p.ash),
                  maxLines: 1,
                ),
              ),
              const SizedBox(width: S.md),
              SizedBox(
                width: 52,
                child: Text(
                  mmss(t.duration),
                  textAlign: TextAlign.right,
                  style: T.caption(p.mute),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
