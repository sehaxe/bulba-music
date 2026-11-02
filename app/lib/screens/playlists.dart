import 'package:flutter/material.dart';

import '../api/client.dart';
import '../design/components.dart';
import '../design/tokens.dart';
import '../player/controller.dart';
import '../player/likes.dart';
import '../widgets/track_menu.dart';
import '../widgets/track_row.dart';

/// Playlists: mine and the public ones, each a card that opens its own page.
class PlaylistsScreen extends StatefulWidget {
  const PlaylistsScreen({
    super.key,
    required this.api,
    required this.player,
    required this.likes,
    required this.onRadio,
    required this.onOpen,
    this.onChanged,
    this.embedded = false,
  });

  final Api api;
  final PlayerController player;
  final LikesStore likes;
  final void Function(Track) onRadio;
  final void Function(Playlist) onOpen;
  final VoidCallback? onChanged;

  /// True when the list lives inside another screen (the liked tab) and must
  /// not paint its own page header.
  final bool embedded;

  @override
  State<PlaylistsScreen> createState() => _PlaylistsScreenState();
}

class _PlaylistsScreenState extends State<PlaylistsScreen> {
  List<Playlist> _items = const [];
  bool _loading = true;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    try {
      final items = await widget.api.playlists();
      if (mounted) {
        setState(() {
          _items = items;
          _loading = false;
        });
      }
    } catch (_) {
      if (mounted) setState(() => _loading = false);
    }
  }

  Future<void> _create() async {
    final name = await promptName(context, 'новый плейлист');
    if (name == null || name.trim().isEmpty) return;
    try {
      final id = await widget.api.createPlaylist(name.trim());
      widget.onChanged?.call();
      await _load();
      if (!mounted) return;
      final created = _items.where((p) => p.id == id);
      if (created.isNotEmpty) widget.onOpen(created.first);
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context)
            .showSnackBar(SnackBar(content: Text('не вышло: $e')));
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    final body = _loading
        ? Center(child: Text('loading…', style: T.label(p.mute)))
        : _items.isEmpty
        ? Center(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                Icon(Icons.queue_music_rounded, size: 34, color: p.ash),
                const SizedBox(height: S.md),
                Text('плейлистов пока нет', style: T.label(p.mute)),
                const SizedBox(height: S.md),
                PillButton(
                  icon: Icons.add_rounded,
                  label: 'создать',
                  onTap: _create,
                ),
              ],
            ),
          )
        : ListView.separated(
            padding: EdgeInsets.fromLTRB(
              S.xl,
              widget.embedded ? S.md : S.sm,
              S.xl,
              S.xxl,
            ),
            itemCount: _items.length,
            separatorBuilder: (_, _) => const SizedBox(height: S.sm),
            itemBuilder: (context, i) => _PlaylistCard(
              playlist: _items[i],
              api: widget.api,
              onOpen: () => widget.onOpen(_items[i]),
              onLike: () async {
                final pl = _items[i];
                await widget.api.likePlaylist(pl.id, on: !pl.liked);
                await _load();
              },
            ),
          );

    if (widget.embedded) return body;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(S.xl, S.xxl, S.xl, S.sm),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.end,
            children: [
              Expanded(child: Text('плейлисты', style: T.displayM(p.ink))),
              PillButton(
                icon: Icons.add_rounded,
                label: 'создать',
                onTap: _create,
              ),
            ],
          ),
        ),
        const SizedBox(height: S.sm),
        Hr(color: p.hairlineStrong),
        Expanded(child: body),
      ],
    );
  }
}

class _PlaylistCard extends StatefulWidget {
  const _PlaylistCard({
    required this.playlist,
    required this.api,
    required this.onOpen,
    required this.onLike,
  });
  final Playlist playlist;
  final Api api;
  final VoidCallback onOpen;
  final VoidCallback onLike;

  @override
  State<_PlaylistCard> createState() => _PlaylistCardState();
}

class _PlaylistCardState extends State<_PlaylistCard> {
  bool _hover = false;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    final accent = AccentScope.of(context);
    final pl = widget.playlist;
    final minutes = (pl.durationMs / 60000).round();
    return MouseRegion(
      cursor: SystemMouseCursors.click,
      onEnter: (_) => setState(() => _hover = true),
      onExit: (_) => setState(() => _hover = false),
      child: GestureDetector(
        onTap: widget.onOpen,
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 150),
          padding: const EdgeInsets.all(S.md),
          decoration: BoxDecoration(
            color: _hover ? p.soft : Colors.transparent,
            border: Border.all(color: p.hairline),
            borderRadius: BorderRadius.circular(12),
          ),
          child: Row(
            children: [
              CoverArt(
                url: pl.hasCover && pl.coverAlbum != null
                    ? widget.api.cover(pl.coverAlbum, true)
                    : null,
                size: 54,
                radius: 10,
                animate: false,
              ),
              const SizedBox(width: S.md),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Row(
                      children: [
                        Flexible(
                          child: Text(
                            pl.name,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: T.bodyStrong(p.ink),
                          ),
                        ),
                        const SizedBox(width: S.sm),
                        Icon(
                          pl.isPublic
                              ? Icons.public_rounded
                              : Icons.lock_outline_rounded,
                          size: 13,
                          color: p.ash,
                        ),
                      ],
                    ),
                    Text(
                      '${pl.tracks} ${pl.tracks == 1 ? 'трек' : 'треков'}'
                      '${minutes > 0 ? ' · $minutes мин' : ''}'
                      '${pl.mine ? '' : ' · ${pl.owner}'}',
                      style: T.micro(p.mute),
                    ),
                  ],
                ),
              ),
              IconButton(
                tooltip: pl.liked ? 'убрать из любимого' : 'в любимое',
                onPressed: widget.onLike,
                icon: Icon(
                  pl.liked
                      ? Icons.favorite_rounded
                      : Icons.favorite_border_rounded,
                  size: 18,
                  color: pl.liked ? accent : p.mute,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// A single playlist page: its tracks, a play button, the visibility switch
/// and removal for the owner.
class PlaylistScreen extends StatefulWidget {
  const PlaylistScreen({
    super.key,
    required this.api,
    required this.player,
    required this.likes,
    required this.playlist,
    required this.onRadio,
    required this.onBack,
    this.onChanged,
  });

  final Api api;
  final PlayerController player;
  final LikesStore likes;
  final Playlist playlist;
  final void Function(Track) onRadio;
  final VoidCallback onBack;
  final VoidCallback? onChanged;

  @override
  State<PlaylistScreen> createState() => _PlaylistScreenState();
}

class _PlaylistScreenState extends State<PlaylistScreen> {
  late Playlist _pl = widget.playlist;
  List<Track> _tracks = const [];
  bool _loading = true;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    try {
      final tracks = await widget.api.playlistTracks(_pl.id);
      final all = await widget.api.playlists();
      final fresh = all.where((p) => p.id == _pl.id);
      if (!mounted) return;
      setState(() {
        _tracks = tracks;
        if (fresh.isNotEmpty) _pl = fresh.first;
        _loading = false;
      });
    } catch (_) {
      if (mounted) setState(() => _loading = false);
    }
  }

  /// One menu instead of three buttons: rename, visibility and delete are all
  /// rare next to play, and every extra chip costs a glyph of attention.
  Future<void> _manage() async {
    final p = Palette.of(context);
    await showModalBottomSheet<void>(
      context: context,
      backgroundColor: Colors.transparent,
      builder: (context) => Container(
        margin: const EdgeInsets.all(S.md),
        decoration: BoxDecoration(
          color: p.roomElevated,
          borderRadius: BorderRadius.circular(16),
        ),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            ListTile(
              leading: Icon(Icons.edit_outlined, color: p.mute),
              title: Text('переименовать', style: T.body(p.ink)),
              onTap: () {
                Navigator.of(context).pop();
                _rename();
              },
            ),
            ListTile(
              leading: Icon(Icons.delete_outline_rounded, color: p.mute),
              title: Text('удалить плейлист', style: T.body(p.ink)),
              onTap: () {
                Navigator.of(context).pop();
                _delete();
              },
            ),
          ],
        ),
      ),
    );
  }

  Future<void> _rename() async {
    final name = await promptName(context, 'название', initial: _pl.name);
    if (name == null || name.trim().isEmpty) return;
    await widget.api.patchPlaylist(_pl.id, name: name.trim());
    await _load();
    widget.onChanged?.call();
  }

  Future<void> _delete() async {
    final p = Palette.of(context);
    final sure = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        backgroundColor: p.roomElevated,
        title: Text('удалить плейлист?', style: T.heading(p.ink)),
        content: Text(
          '«${_pl.name}» исчезнет, треки останутся в библиотеке.',
          style: T.body(p.mute),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: Text('нет', style: T.label(p.mute)),
          ),
          TextButton(
            onPressed: () => Navigator.of(context).pop(true),
            child: Text('удалить', style: T.label(AccentScope.of(context))),
          ),
        ],
      ),
    );
    if (sure != true) return;
    await widget.api.deletePlaylist(_pl.id);
    widget.onChanged?.call();
    widget.onBack();
  }

  /// The new callback already adjusts the target index for the removed row.
  Future<void> _reorderItem(int from, int to) async {
    final next = [..._tracks];
    final moved = next.removeAt(from);
    next.insert(to, moved);
    setState(() => _tracks = next);
    try {
      await widget.api.reorderPlaylist(_pl.id, [for (final t in next) t.id]);
    } catch (_) {
      // The server refused; keep the local order but say so instead of lying.
      if (mounted) {
        ScaffoldMessenger.of(
          context,
        ).showSnackBar(const SnackBar(content: Text('порядок не сохранился')));
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    final accent = AccentScope.of(context);
    final minutes = (_pl.durationMs / 60000).round();

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(S.xl, S.xl, S.xl, S.sm),
          child: Row(
            children: [
              _Chip(icon: Icons.arrow_back_rounded, onTap: widget.onBack),
              const SizedBox(width: S.md),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      _pl.name,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: T.displayM(p.ink),
                    ),
                    Text(
                      '${_pl.tracks} треков · $minutes мин · '
                      '${_pl.isPublic ? 'публичный' : 'локальный'}',
                      style: T.caption(p.mute),
                    ),
                  ],
                ),
              ),
              PillButton(
                icon: Icons.play_arrow_rounded,
                label: 'play',
                onTap: _tracks.isEmpty
                    ? null
                    : () => widget.player.play(_tracks.first, queue: _tracks),
              ),
              const SizedBox(width: S.sm),
              _Chip(
                icon: _pl.liked
                    ? Icons.favorite_rounded
                    : Icons.favorite_border_rounded,
                color: _pl.liked ? accent : null,
                onTap: () async {
                  await widget.api.likePlaylist(_pl.id, on: !_pl.liked);
                  await _load();
                  widget.onChanged?.call();
                },
              ),
              if (_pl.mine) ...[
                const SizedBox(width: S.sm),
                _Chip(
                  icon: Icons.more_vert_rounded,
                  tooltip: 'ещё',
                  onTap: _manage,
                ),
                const SizedBox(width: S.sm),
                _Chip(
                  icon: _pl.isPublic
                      ? Icons.public_rounded
                      : Icons.lock_outline_rounded,
                  tooltip: _pl.isPublic
                      ? 'сделать локальным'
                      : 'сделать публичным',
                  onTap: () async {
                    await widget.api.patchPlaylist(
                      _pl.id,
                      isPublic: !_pl.isPublic,
                    );
                    await _load();
                    widget.onChanged?.call();
                  },
                ),
              ],
            ],
          ),
        ),
        Hr(color: p.hairlineStrong),
        Expanded(
          child: _loading
              ? Center(child: Text('loading…', style: T.label(p.mute)))
              : _tracks.isEmpty
              ? Center(
                  child: Text(
                    'пусто — добавь треки через меню трека',
                    style: T.caption(p.mute),
                  ),
                )
              : ReorderableListView.builder(
                  padding: const EdgeInsets.fromLTRB(S.xl, S.sm, S.xl, S.xxl),
                  itemCount: _tracks.length,
                  buildDefaultDragHandles: false,
                  onReorderItem: _reorderItem,
                  itemBuilder: (context, i) {
                    final t = _tracks[i];
                    return TrackRow(
                      key: ValueKey('pl-${_pl.id}-${t.id}'),
                      track: t,
                      index: i + 1,
                      leading: _pl.mine
                          ? ReorderableDragStartListener(
                              index: i,
                              child: MouseRegion(
                                cursor: SystemMouseCursors.grab,
                                child: Icon(
                                  Icons.drag_indicator_rounded,
                                  size: 16,
                                  color: Palette.of(context).ash,
                                ),
                              ),
                            )
                          : null,
                      playing: widget.player.current?.id == t.id,
                      coverUrl: widget.api.cover(t.albumId, t.hasCover, w: 128),
                      onTap: () => widget.player.play(t, queue: _tracks),
                      onRadio: () => widget.onRadio(t),
                      onMenu: () => showTrackMenu(
                        context,
                        track: t,
                        api: widget.api,
                        player: widget.player,
                        likes: widget.likes,
                        onRadio: () => widget.onRadio(t),
                        removeFrom: _pl.mine ? _pl.id : null,
                        onRemoved: _load,
                      ),
                      liked: widget.likes.contains(t.id),
                      onLike: () => widget.likes.toggle(t),
                    );
                  },
                ),
        ),
      ],
    );
  }
}

class _Chip extends StatelessWidget {
  const _Chip({
    required this.icon,
    required this.onTap,
    this.color,
    this.tooltip,
  });
  final IconData icon;
  final VoidCallback onTap;
  final Color? color;
  final String? tooltip;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    final chip = MouseRegion(
      cursor: SystemMouseCursors.click,
      child: GestureDetector(
        onTap: onTap,
        child: Container(
          padding: const EdgeInsets.all(8),
          decoration: BoxDecoration(
            border: Border.all(color: p.hairline),
            borderRadius: BorderRadius.circular(9),
          ),
          child: Icon(icon, size: 16, color: color ?? p.mute),
        ),
      ),
    );
    return tooltip == null ? chip : Tooltip(message: tooltip!, child: chip);
  }
}

/// Name dialog shared by creation and renaming.
Future<String?> promptName(
  BuildContext context,
  String title, {
  String? initial,
}) {
  final controller = TextEditingController(text: initial ?? '');
  final p = Palette.of(context);
  return showDialog<String>(
    context: context,
    builder: (context) => AlertDialog(
      backgroundColor: p.roomElevated,
      title: Text(title, style: T.heading(p.ink)),
      content: TextField(
        controller: controller,
        autofocus: true,
        cursorColor: AccentScope.of(context),
        style: T.body(p.ink),
        onSubmitted: (v) => Navigator.of(context).pop(v),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text('отмена', style: T.label(p.mute)),
        ),
        TextButton(
          onPressed: () => Navigator.of(context).pop(controller.text),
          child: Text('ок', style: T.label(AccentScope.of(context))),
        ),
      ],
    ),
  );
}

/// Picker used by the track sheet: add the track to one of the playlists, or
/// start a new one right here.
Future<void> showAddToPlaylist(
  BuildContext context, {
  required Api api,
  required Track track,
}) async {
  final items = await api.playlists();
  if (!context.mounted) return;
  await showModalBottomSheet<void>(
    context: context,
    backgroundColor: Colors.transparent,
    builder: (context) {
      final p = Palette.of(context);
      final accent = AccentScope.of(context);
      return Container(
        decoration: BoxDecoration(
          color: p.roomElevated,
          borderRadius: const BorderRadius.vertical(top: Radius.circular(18)),
          border: Border.all(color: p.hairline),
        ),
        child: SafeArea(
          top: false,
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Padding(
                padding: const EdgeInsets.fromLTRB(S.lg, S.lg, S.lg, S.sm),
                child: Row(
                  children: [
                    Text('в плейлист', style: T.heading(p.ink)),
                    const Spacer(),
                    TextButton.icon(
                      onPressed: () async {
                        final name = await promptName(
                          context,
                          'новый плейлист',
                        );
                        if (name == null || name.trim().isEmpty) return;
                        final id = await api.createPlaylist(name.trim());
                        await api.addToPlaylist(id, track.id);
                        if (context.mounted) Navigator.of(context).pop();
                      },
                      icon: Icon(Icons.add_rounded, size: 16, color: accent),
                      label: Text('создать', style: T.label(accent)),
                    ),
                  ],
                ),
              ),
              Hr(color: p.hairline),
              ConstrainedBox(
                constraints: const BoxConstraints(maxHeight: 340),
                child: items.isEmpty
                    ? Padding(
                        padding: const EdgeInsets.all(S.xl),
                        child: Text(
                          'плейлистов ещё нет',
                          style: T.caption(p.mute),
                        ),
                      )
                    : ListView(
                        shrinkWrap: true,
                        children: [
                          for (final pl in items)
                            ListTile(
                              dense: true,
                              leading: Icon(
                                pl.isPublic
                                    ? Icons.public_rounded
                                    : Icons.lock_outline_rounded,
                                size: 16,
                                color: p.ash,
                              ),
                              title: Text(pl.name, style: T.body(p.ink)),
                              subtitle: Text(
                                '${pl.tracks} треков',
                                style: T.micro(p.mute),
                              ),
                              trailing: Icon(
                                pl.mine
                                    ? Icons.add_rounded
                                    : Icons.lock_rounded,
                                size: 16,
                                color: p.mute,
                              ),
                              onTap: pl.mine
                                  ? () async {
                                      await api.addToPlaylist(pl.id, track.id);
                                      if (context.mounted) {
                                        Navigator.of(context).pop();
                                      }
                                    }
                                  : null,
                            ),
                        ],
                      ),
              ),
            ],
          ),
        ),
      );
    },
  );
}
