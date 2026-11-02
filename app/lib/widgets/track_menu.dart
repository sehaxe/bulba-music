import 'package:flutter/material.dart';

import '../api/client.dart';
import '../design/components.dart';
import '../design/tokens.dart';
import '../design/badges.dart';
import '../screens/playlists.dart';
import '../player/controller.dart';
import '../player/likes.dart';

/// The one place a track can be interrogated: what it is, what it plays from,
/// and what to do with it next. Both the long-press and the right click land
/// here, so there is no second, poorer menu anywhere in the app.
Future<void> showTrackMenu(
  BuildContext context, {
  required Track track,
  required Api api,
  required PlayerController player,
  required LikesStore likes,
  required VoidCallback onRadio,

  /// Set when the sheet is opened from inside one of the user's playlists:
  /// removing a track belongs there, not in a global menu everywhere.
  int? removeFrom,
  VoidCallback? onRemoved,
}) {
  return showModalBottomSheet<void>(
    context: context,
    isScrollControlled: true,
    backgroundColor: Colors.transparent,
    barrierColor: Colors.black54,
    builder: (_) => _TrackSheet(
      track: track,
      api: api,
      player: player,
      likes: likes,
      onRadio: onRadio,
      removeFrom: removeFrom,
      onRemoved: onRemoved,
    ),
  );
}

class _TrackSheet extends StatefulWidget {
  const _TrackSheet({
    required this.track,
    required this.api,
    required this.player,
    required this.likes,
    required this.onRadio,
    this.removeFrom,
    this.onRemoved,
  });
  final Track track;
  final Api api;
  final PlayerController player;
  final LikesStore likes;
  final VoidCallback onRadio;
  final int? removeFrom;
  final VoidCallback? onRemoved;

  @override
  State<_TrackSheet> createState() => _TrackSheetState();
}

class _TrackSheetState extends State<_TrackSheet> {
  @override
  void initState() {
    super.initState();
    widget.likes.addListener(_rebuild);
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
    final t = widget.track;
    final liked = widget.likes.contains(t.id);

    return Container(
      decoration: BoxDecoration(
        color: p.roomElevated,
        borderRadius: const BorderRadius.vertical(top: Radius.circular(20)),
        border: Border.all(color: p.hairline),
      ),
      padding: const EdgeInsets.only(bottom: S.lg),
      child: SafeArea(
        top: false,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            SizedBox(
              height: 26,
              child: Center(
                child: Container(
                  width: 36,
                  height: 4,
                  margin: const EdgeInsets.only(top: 10),
                  decoration: BoxDecoration(
                    color: p.hairlineStrong,
                    borderRadius: BorderRadius.circular(2),
                  ),
                ),
              ),
            ),
            Padding(
              padding: const EdgeInsets.fromLTRB(S.lg, S.sm, S.lg, S.md),
              child: Row(
                children: [
                  CoverArt(
                    url: widget.api.cover(t.albumId, t.hasCover),
                    size: 76,
                    radius: 12,
                  ),
                  const SizedBox(width: S.md),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Text(
                          t.title,
                          maxLines: 2,
                          overflow: TextOverflow.ellipsis,
                          style: T.bodyStrong(p.ink),
                        ),
                        const SizedBox(height: 2),
                        Text(
                          '${t.artist} · ${t.album}',
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: T.caption(p.mute),
                        ),
                        const SizedBox(height: 4),
                        Text(
                          [
                            if (t.year != null) '${t.year}',
                            mmss(t.duration),
                            t.format,
                          ].join(' · '),
                          style: T.micro(p.ash),
                        ),
                      ],
                    ),
                  ),
                ],
              ),
            ),
            Hr(color: p.hairline),
            _Item(
              icon: Icons.playlist_play_rounded,
              label: 'играть следующим',
              onTap: () {
                widget.player.playNext(t);
                Navigator.of(context).pop();
              },
            ),
            _Item(
              icon: Icons.queue_music_rounded,
              label: 'в очередь',
              onTap: () {
                widget.player.addToQueue(t);
                Navigator.of(context).pop();
              },
            ),
            _Item(
              icon: Icons.playlist_add_rounded,
              label: 'добавить в плейлист',
              onTap: () {
                Navigator.of(context).pop();
                showAddToPlaylist(context, api: widget.api, track: t);
              },
            ),
            _Item(
              icon: Icons.auto_awesome_rounded,
              label: 'радио от этого трека',
              onTap: () {
                Navigator.of(context).pop();
                widget.onRadio();
              },
            ),
            _Item(
              icon: liked
                  ? Icons.favorite_rounded
                  : Icons.favorite_border_rounded,
              label: liked ? 'убрать из любимого' : 'в любимое',
              accent: liked,
              onTap: () => widget.likes.toggle(t),
            ),
            if (widget.removeFrom != null)
              _Item(
                icon: Icons.playlist_remove_rounded,
                label: 'убрать из плейлиста',
                onTap: () async {
                  await widget.api.removeFromPlaylist(widget.removeFrom!, t.id);
                  if (!context.mounted) return;
                  Navigator.of(context).pop();
                  widget.onRemoved?.call();
                },
              ),
            _Item(
              icon: Icons.info_outline_rounded,
              label: 'подробнее',
              onTap: () {
                Navigator.of(context).pop();
                _showDetails(context, t, widget.api);
              },
            ),
          ],
        ),
      ),
    );
  }
}

void _showDetails(BuildContext context, Track t, Api api) {
  final p = Palette.of(context);
  showDialog<void>(
    context: context,
    builder: (context) => Dialog(
      backgroundColor: p.roomElevated,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(16)),
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 420),
        child: Padding(
          padding: const EdgeInsets.all(S.xl),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text('детали', style: T.heading(p.ink)),
              const SizedBox(height: S.md),
              _DetailRow(label: 'название', value: t.title, p: p),
              _DetailRow(label: 'артист', value: t.artist, p: p),
              _DetailRow(label: 'альбом', value: t.album, p: p),
              if (t.year != null)
                _DetailRow(label: 'год', value: '${t.year}', p: p),
              if (t.disc != null)
                _DetailRow(label: 'диск', value: '${t.disc}', p: p),
              if (t.trackNo != null)
                _DetailRow(label: 'трек', value: '${t.trackNo}', p: p),
              _DetailRow(label: 'длительность', value: mmss(t.duration), p: p),
              if (tagsOf(t).isNotEmpty)
                _DetailRow(
                  label: 'теги',
                  value: tagsOf(t).map((e) => e.label).join(' · '),
                  p: p,
                ),
              _DetailRow(
                label: 'формат',
                value: [
                  t.codec ?? '—',
                  if (t.bits != null) '${t.bits} bit',
                  if (t.sampleRate != null)
                    '${(t.sampleRate! / 1000).toStringAsFixed(1)} kHz',
                  if (t.bitrate != null) '${t.bitrate} kbps',
                ].join(' · '),
                p: p,
              ),
              if (t.bytes != null)
                _DetailRow(
                  label: 'размер',
                  value: '${(t.bytes! / 1048576).toStringAsFixed(1)} МБ',
                  p: p,
                ),
              const SizedBox(height: S.md),
              Align(
                alignment: Alignment.centerRight,
                child: TextButton(
                  onPressed: () => Navigator.of(context).pop(),
                  child: Text(
                    'закрыть',
                    style: T.label(AccentScope.of(context)),
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
    ),
  );
}

class _DetailRow extends StatelessWidget {
  const _DetailRow({required this.label, required this.value, required this.p});
  final String label;
  final String value;
  final Palette p;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 3),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SizedBox(width: 110, child: Text(label, style: T.caption(p.mute))),
          Expanded(child: Text(value, style: T.body(p.ink))),
        ],
      ),
    );
  }
}

class _Item extends StatefulWidget {
  const _Item({
    required this.icon,
    required this.label,
    required this.onTap,
    this.accent = false,
  });
  final IconData icon;
  final String label;
  final VoidCallback onTap;
  final bool accent;

  @override
  State<_Item> createState() => _ItemState();
}

class _ItemState extends State<_Item> {
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
          color: _hover ? p.card : Colors.transparent,
          padding: const EdgeInsets.symmetric(horizontal: S.lg, vertical: 13),
          child: Row(
            children: [
              Icon(
                widget.icon,
                size: 18,
                color: widget.accent ? AccentScope.of(context) : p.onRoomMute,
              ),
              const SizedBox(width: S.md),
              Text(widget.label, style: T.body(p.onRoom)),
            ],
          ),
        ),
      ),
    );
  }
}
