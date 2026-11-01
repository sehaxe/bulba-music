import 'package:flutter/material.dart';

import 'api/client.dart';
import 'design/accent.dart';
import 'design/components.dart';
import 'design/tokens.dart';
import 'player/controller.dart';
import 'screens/home.dart';
import 'screens/library.dart';
import 'screens/player_screen.dart';
import 'screens/radio.dart';
import 'widgets/nav.dart';
import 'widgets/player_bar.dart';

class Shell extends StatefulWidget {
  const Shell({super.key, required this.api, required this.player});
  final Api api;
  final PlayerController player;

  @override
  State<Shell> createState() => _ShellState();
}

class _ShellState extends State<Shell> {
  List<Track> _tracks = const [];
  String? _error;
  bool _loading = true;
  int _rev = 0, _albums = 0, _artists = 0;
  int _tab = 0;
  Color _accent = C.signal;
  int? _accentTrack;

  @override
  void initState() {
    super.initState();
    widget.player.addListener(_onPlayer);
    _load();
  }

  @override
  void dispose() {
    widget.player.removeListener(_onPlayer);
    super.dispose();
  }

  void _onPlayer() {
    final t = widget.player.current;
    if (t != null && t.id != _accentTrack) {
      _accentTrack = t.id;
      final url = widget.api.cover(t.albumId, t.hasCover);
      AccentCache.of(url).then((c) {
        if (mounted) setState(() => _accent = c);
      });
    }
    setState(() {});
  }

  Future<void> _load() async {
    setState(() {
      _loading = true;
      _error = null;
    });
    try {
      final tracks = await widget.api.tracks();
      final health = await widget.api.health();
      if (!mounted) return;
      setState(() {
        _tracks = tracks;
        _rev = (health['rev'] ?? 0) as int;
        _albums = (health['albums'] ?? 0) as int;
        _artists = (health['artists'] ?? 0) as int;
        _loading = false;
      });
      if (const bool.fromEnvironment('SHOT_PLAYER') && tracks.isNotEmpty) {
        await widget.player.play(tracks.first, queue: tracks);
        if (mounted) _openPlayer();
      }
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _error = e.toString();
        _loading = false;
      });
    }
  }

  Future<void> _rescan() async {
    setState(() => _loading = true);
    try {
      await widget.api.scan();
      await widget.api.buildFeatures(limit: 500);
      await _load();
    } catch (e) {
      if (mounted) {
        setState(() {
          _loading = false;
          _error = e.toString();
        });
      }
    }
  }

  Future<void> _startRadio(Track seed) async {
    try {
      final similar = await widget.api.similar(seed.id, n: 40);
      if (similar.isEmpty) return;
      await widget.player.play(similar.first, queue: similar);
      if (mounted) {
        setState(() => _tab = 2);
        if (MediaQuery.sizeOf(context).width < 900) _openPlayer();
      }
    } catch (e) {
      debugPrint('radio failed: $e');
    }
  }

  void _openPlayer() {
    if (widget.player.current == null) return;
    Navigator.of(context).push(
      PixelPageRoute<void>(
        builder: (_) => PlayerScreen(
          player: widget.player,
          api: widget.api,
          onRadio: _startRadio,
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    final size = MediaQuery.sizeOf(context);
    final wide = size.width >= 900;

    final body = Column(
      children: [
        Expanded(child: _screen()),
        Hr(color: p.hairline),
        _StatusBar(
          tracks: _tracks.length,
          albums: _albums,
          artists: _artists,
          rev: _rev,
          error: _error,
          loading: _loading,
        ),
      ],
    );

    return Scaffold(
      backgroundColor: p.canvas,
      body: SafeArea(
        bottom: !wide,
        child: Column(
          children: [
            Expanded(
              child: wide
                  ? Row(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: [
                        NavRail(
                          index: _tab,
                          onSelect: _select,
                          accent: _accent,
                        ),
                        Container(width: 1, color: p.hairline),
                        Expanded(child: body),
                      ],
                    )
                  : body,
            ),
            if (widget.player.current != null)
              PlayerBar(
                player: widget.player,
                api: widget.api,
                onOpen: _openPlayer,
                accent: _accent,
                height: wide ? 68 : 60,
                showVolume: wide,
              ),
            if (!wide)
              FloatingNav(index: _tab, onSelect: _select, accent: _accent),
          ],
        ),
      ),
    );
  }

  void _select(int i) => setState(() => _tab = i);

  Widget _screen() {
    final p = Palette.of(context);
    if (_loading) {
      return Center(child: Text('[ loading ]', style: T.label(p.mute)));
    }
    if (_error != null) {
      return Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text('[!] server unreachable', style: T.heading(p.ink)),
            const SizedBox(height: S.sm),
            SizedBox(
              width: 420,
              child: Text(
                _error!,
                textAlign: TextAlign.center,
                style: T.caption(p.mute),
              ),
            ),
            const SizedBox(height: S.lg),
            BracketButton('retry', onTap: _load),
          ],
        ),
      );
    }
    if (_tracks.isEmpty) {
      return Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text('library is empty', style: T.displayM(p.mute)),
            const SizedBox(height: S.sm),
            Text(
              'drop flac into the library folder, then rescan',
              style: T.caption(p.ash),
            ),
          ],
        ),
      );
    }
    switch (_tab) {
      case 0:
        return HomeScreen(
          tracks: _tracks,
          api: widget.api,
          player: widget.player,
          onRadio: _startRadio,
          onRescan: _rescan,
        );
      case 1:
        return LibraryScreen(
          tracks: _tracks,
          api: widget.api,
          player: widget.player,
          onRadio: _startRadio,
        );
      case 2:
        return RadioScreen(
          player: widget.player,
          api: widget.api,
          onRadio: _startRadio,
        );
      case 3:
        return const PlaceholderScreen(
          title: 'downloads',
          note: 'offline cache and sync',
        );
      default:
        return const PlaceholderScreen(
          title: 'settings',
          note: 'server, crossfade, quality',
        );
    }
  }
}

class _StatusBar extends StatelessWidget {
  const _StatusBar({
    required this.tracks,
    required this.albums,
    required this.artists,
    required this.rev,
    required this.error,
    required this.loading,
  });

  final int tracks;
  final int albums;
  final int artists;
  final int rev;
  final String? error;
  final bool loading;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    final state = error != null
        ? 'offline'
        : loading
        ? 'scanning'
        : 'online';
    return Container(
      height: 24,
      padding: const EdgeInsets.symmetric(horizontal: S.lg),
      child: Row(
        children: [
          Text(
            '$tracks tracks  ·  $albums albums  ·  $artists artists  ·  rev $rev',
            style: T.micro(p.ash),
          ),
          const Spacer(),
          Container(
            width: 6,
            height: 6,
            decoration: BoxDecoration(
              color: error != null ? C.danger : const Color(0xFF30D158),
              shape: BoxShape.circle,
            ),
          ),
          const SizedBox(width: S.sm),
          Text(state, style: T.micro(p.ash)),
        ],
      ),
    );
  }
}
