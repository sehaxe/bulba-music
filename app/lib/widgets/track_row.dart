import 'package:flutter/material.dart';

import '../api/client.dart';
import '../design/components.dart';
import '../design/tokens.dart';
import '../design/badges.dart';

class TrackRow extends StatefulWidget {
  const TrackRow({
    super.key,
    required this.track,
    required this.playing,
    required this.coverUrl,
    required this.onTap,
    this.onRadio,
    this.showCover = true,
    this.index,
    this.liked = false,
    this.onLike,
    this.onMenu,
    this.subtitle,
  });

  final Track track;
  final bool playing;
  final String? coverUrl;
  final VoidCallback onTap;
  final VoidCallback? onRadio;
  final bool showCover;
  final int? index;
  final bool liked;
  final VoidCallback? onLike;

  /// Opens the shared track sheet (details, play next, radio, like).
  final VoidCallback? onMenu;

  /// Overrides the artist/album line — an album page does not need to repeat
  /// its own name on all thirteen rows.
  final String? subtitle;

  @override
  State<TrackRow> createState() => _TrackRowState();
}

class _TrackRowState extends State<TrackRow> {
  bool _hover = false;

  void _menu() {
    if (widget.onMenu != null) {
      widget.onMenu!();
    } else if (widget.onRadio != null) {
      widget.onRadio!();
    }
  }

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
        onLongPress: (widget.onMenu ?? widget.onRadio) == null ? null : _menu,
        onSecondaryTapDown: (widget.onMenu ?? widget.onRadio) == null
            ? null
            : (_) => _menu(),
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 90),
          curve: Curves.easeOut,
          margin: const EdgeInsets.symmetric(horizontal: S.xs),
          decoration: BoxDecoration(
            color: widget.playing
                ? p.card
                : _hover
                ? p.soft
                : Colors.transparent,
            borderRadius: BorderRadius.circular(10),
          ),
          padding: EdgeInsets.symmetric(
            horizontal: MediaQuery.sizeOf(context).width < 520 ? S.md : S.xl,
            vertical: S.sm,
          ),
          child: Builder(
            builder: (context) {
              // On a phone the index, the format badge and the duration used to
              // eat more than half the row, leaving "Let…" instead of a title.
              final narrow = MediaQuery.sizeOf(context).width < 520;
              return Row(
                children: [
                  if (widget.index != null && !narrow) ...[
                    SizedBox(
                      width: 26,
                      child: Text(
                        widget.playing ? '▸' : '${widget.index}',
                        textAlign: TextAlign.right,
                        style: T.caption(
                          widget.playing ? AccentScope.of(context) : p.ash,
                        ),
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
                          style: T.bodyStrong(
                            widget.playing ? AccentScope.of(context) : p.ink,
                          ),
                        ),
                        Row(
                          children: [
                            Flexible(
                              child: Text(
                                widget.subtitle ??
                                    (widget.showCover
                                        ? t.artist
                                        : '${t.artist} · ${t.album}'),
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                                style: T.caption(p.mute),
                              ),
                            ),
                            const SizedBox(width: 6),
                            Flexible(
                              child: TagStrip(
                                track: t,
                                max: narrow ? 2 : 4,
                                dense: true,
                                color: p.ash,
                              ),
                            ),
                          ],
                        ),
                      ],
                    ),
                  ),
                  if (!narrow) ...[
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
                  ] else
                    const SizedBox(width: S.sm),
                  SizedBox(
                    width: 52,
                    child: Text(
                      mmss(t.duration),
                      textAlign: TextAlign.right,
                      style: T.caption(p.mute),
                    ),
                  ),
                  if (widget.onLike != null) ...[
                    const SizedBox(width: S.xs),
                    GestureDetector(
                      behavior: HitTestBehavior.opaque,
                      onTap: widget.onLike,
                      child: SizedBox(
                        width: 32,
                        height: 32,
                        child: AnimatedScale(
                          scale: widget.liked ? 1.0 : 0.92,
                          duration: const Duration(milliseconds: 140),
                          child: Icon(
                            widget.liked
                                ? Icons.favorite_rounded
                                : Icons.favorite_border_rounded,
                            size: 17,
                            color: widget.liked
                                ? AccentScope.of(context)
                                : (_hover ? p.mute : Colors.transparent),
                          ),
                        ),
                      ),
                    ),
                  ],
                ],
              );
            },
          ),
        ),
      ),
    );
  }
}
