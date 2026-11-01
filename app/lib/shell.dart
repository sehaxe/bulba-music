import 'dart:io';

import 'package:flutter/material.dart';

import 'api/client.dart';
import 'design/accent.dart';
import 'design/components.dart';
import 'design/tokens.dart';
import 'player/controller.dart';
import 'screens/collection.dart';
import 'screens/home.dart';
import 'screens/library.dart';
import 'screens/player_screen.dart';
import 'screens/radio.dart';
import 'screens/search.dart';
import 'screens/settings.dart';
import 'widgets/nav.dart';
import 'widgets/player_bar.dart';

class Shell extends StatefulWidget {
  const Shell({
    super.key,
    required this.api,
    required this.player,
    required this.themeMode,
  });
  final Api api;
  final PlayerController player;
  final ValueNotifier<ThemeMode> themeMode;

  @override
  State<Shell> createState() => _ShellState();
}

class _ShellState extends State<Shell> {
  List<Track> _tracks = const [];
  String? _error;
  bool _loading = true;
  int _tab = 0;
  Color _accent = C.signal;
  int? _accentTrack;
  Collection? _collection;

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
      if (!mounted) return;
      setState(() {
        _tracks = tracks;
        _loading = false;
      });
      final autoPlay =
          const bool.fromEnvironment('SHOT_PLAYER') ||
          Platform.environment['BULBA_AUTOPLAY'] != null;
      if (Platform.environment['BULBA_OPEN_COLLECTION'] != null) {
        final mixes = await widget.api.dailyMixes();
        if (mixes.isNotEmpty && mounted) {
          final mix = mixes.first;
          final covers = <String?>[];
          for (final t in mix.tracks) {
            final url = widget.api.cover(t.albumId, t.hasCover);
            if (url != null && !covers.contains(url)) covers.add(url);
            if (covers.length == 4) break;
          }
          setState(
            () => _collection = Collection(
              title: mix.title,
              subtitle: mix.note,
              tracks: mix.tracks,
              mosaic: covers,
            ),
          );
        }
      }
      if (autoPlay && tracks.isNotEmpty) {
        await widget.player.play(tracks.first, queue: tracks);
        final barOnly = Platform.environment['BULBA_AUTOPLAY'] == 'bar';
        if (mounted && !barOnly) _openPlayer();
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
        if (_error != null && !_loading)
          Container(
            width: double.infinity,
            color: C.danger.withValues(alpha: 0.12),
            padding: const EdgeInsets.symmetric(horizontal: S.lg, vertical: 6),
            child: Text(
              'offline · $_error',
              style: T.micro(C.danger),
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
            ),
          ),
        Expanded(child: _screen()),
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

  void _select(int i) => setState(() {
    _tab = i;
    _collection = null;
  });

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
    final collection = _collection;
    if (collection != null) {
      return CollectionView(
        collection: collection,
        api: widget.api,
        player: widget.player,
        onRadio: _startRadio,
        onBack: () => setState(() => _collection = null),
        accent: _accent,
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
          accent: _accent,
          onOpen: (collection) => setState(() => _collection = collection),
        );
      case 1:
        return SearchScreen(
          api: widget.api,
          player: widget.player,
          onRadio: _startRadio,
        );
      case 2:
        return LibraryScreen(
          tracks: _tracks,
          api: widget.api,
          player: widget.player,
          onRadio: _startRadio,
        );
      case 3:
        return RadioScreen(
          player: widget.player,
          api: widget.api,
          onRadio: _startRadio,
        );
      default:
        return SettingsScreen(
          api: widget.api,
          themeMode: widget.themeMode,
          onRescan: _rescan,
        );
    }
  }
}

class _WindowControl extends StatefulWidget {
  @override
  State<_WindowControl> createState() => _WindowControlState();
}

class _WindowControlState extends State<_WindowControl> {
  bool _hover = false;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    return MouseRegion(
      cursor: SystemMouseCursors.click,
      onEnter: (_) => setState(() => _hover = true),
      onExit: (_) => setState(() => _hover = false),
      child: GestureDetector(
        onTap: () => exit(0),
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 120),
          width: 30,
          height: 30,
          decoration: BoxDecoration(
            color: _hover ? C.danger : Colors.transparent,
            borderRadius: BorderRadius.circular(8),
          ),
          child: Icon(
            Icons.close_rounded,
            size: 17,
            color: _hover ? Colors.white : p.ash,
          ),
        ),
      ),
    );
  }
}
