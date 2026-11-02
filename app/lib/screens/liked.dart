import 'dart:io';

import 'package:flutter/material.dart';

import '../api/client.dart';
import '../design/components.dart';
import '../design/tokens.dart';
import 'playlists.dart';
import '../player/controller.dart';
import '../player/likes.dart';
import '../widgets/track_menu.dart';
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
    required this.onOpenPlaylist,
  });

  final Api api;
  final PlayerController player;
  final LikesStore likes;
  final void Function(Track) onRadio;
  final void Function(List<Track>, String) onOpenCollection;
  final void Function(Playlist) onOpenPlaylist;

  @override
  State<LikedScreen> createState() => _LikedScreenState();
}

class _LikedScreenState extends State<LikedScreen> {
  /// 0 tracks, 1 playlists. Playlists live here because a liked playlist is
  /// the same kind of thing as a liked track: something you marked.
  int _mode = int.tryParse(Platform.environment['BULBA_LIKED_MODE'] ?? '') ?? 0;
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
          child: Wrap(
            alignment: WrapAlignment.spaceBetween,
            crossAxisAlignment: WrapCrossAlignment.end,
            spacing: S.md,
            runSpacing: S.sm,
            children: [
              ConstrainedBox(
                constraints: BoxConstraints(
                  minWidth: MediaQuery.sizeOf(context).width > 560
                      ? MediaQuery.sizeOf(context).width - 420
                      : 200,
                ),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text('любимое', style: T.displayM(p.ink)),
                    const SizedBox(height: S.xxs),
                    Text(
                      _mode == 0
                          ? '${tracks.length} ${plural(tracks.length)}'
                          : 'треки и плейлисты',
                      style: T.caption(p.mute),
                    ),
                  ],
                ),
              ),
              _ModeSwitch(
                value: _mode,
                onChanged: (v) => setState(() => _mode = v),
              ),
              const SizedBox(width: S.sm),
              if (_mode == 0 && tracks.isNotEmpty)
                PillButton(
                  icon: Icons.play_arrow_rounded,
                  label: 'играть',
                  onTap: () => widget.player.play(tracks.first, queue: tracks),
                ),
            ],
          ),
        ),
        const SizedBox(height: S.md),
        Hr(color: p.hairlineStrong),
        Expanded(
          child: _mode == 1
              ? PlaylistsScreen(
                  api: widget.api,
                  player: widget.player,
                  likes: widget.likes,
                  onRadio: widget.onRadio,
                  onOpen: widget.onOpenPlaylist,
                  embedded: true,
                )
              : widget.likes.loading
              ? Center(child: Text('загружаю…', style: T.label(p.mute)))
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
                      coverUrl: widget.api.cover(t.albumId, t.hasCover, w: 128),
                      onTap: () => widget.player.play(t, queue: tracks),
                      onRadio: () => widget.onRadio(t),
                      onMenu: () => showTrackMenu(
                        context,
                        track: t,
                        api: widget.api,
                        player: widget.player,
                        likes: widget.likes,
                        onRadio: () => widget.onRadio(t),
                      ),
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

class _ModeSwitch extends StatelessWidget {
  const _ModeSwitch({required this.value, required this.onChanged});
  final int value;
  final ValueChanged<int> onChanged;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    const labels = ['треки', 'плейлисты'];
    return Container(
      padding: const EdgeInsets.all(3),
      decoration: BoxDecoration(
        color: p.soft,
        border: Border.all(color: p.hairline),
        borderRadius: BorderRadius.circular(10),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          for (var i = 0; i < labels.length; i++)
            GestureDetector(
              onTap: () => onChanged(i),
              child: AnimatedContainer(
                duration: const Duration(milliseconds: 160),
                padding: const EdgeInsets.symmetric(
                  horizontal: 12,
                  vertical: 6,
                ),
                decoration: BoxDecoration(
                  color: value == i ? p.card : Colors.transparent,
                  borderRadius: BorderRadius.circular(8),
                ),
                child: Text(
                  labels[i],
                  style: T.label(value == i ? p.ink : p.mute),
                ),
              ),
            ),
        ],
      ),
    );
  }
}
