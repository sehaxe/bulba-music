import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'api/client.dart';
import 'cast/cast_sheet.dart';
import 'cast/cast_store.dart';
import 'design/accent.dart';
import 'design/components.dart';
import 'design/tokens.dart';
import 'player/controller.dart';
import 'player/downloads.dart';
import 'player/likes.dart';
import 'state/prefs.dart';
import 'state/profiles.dart';
import 'screens/collection.dart';
import 'screens/home.dart';
import 'screens/library.dart';
import 'screens/player_screen.dart';
import 'screens/duplicates.dart';
import 'screens/playlists.dart';
import 'screens/wave.dart';
import 'screens/liked.dart';
import 'screens/album.dart';
import 'screens/artist.dart';
import 'screens/search.dart';
import 'screens/settings.dart';
import 'widgets/nav.dart';
import 'widgets/player_bar.dart';
import 'widgets/queue_sheet.dart';

class Shell extends StatefulWidget {
  const Shell({
    super.key,
    required this.api,
    required this.player,
    required this.themeMode,
    required this.likes,
    required this.prefs,
    required this.downloads,
    required this.cast,
    required this.profiles,
    required this.onProfileChanged,
  });
  final Api api;
  final PlayerController player;
  final LikesStore likes;
  final Prefs prefs;
  final Downloads downloads;
  final CastStore cast;
  final Profiles profiles;
  final Future<void> Function(String phrase) onProfileChanged;
  final ValueNotifier<ThemeMode> themeMode;

  @override
  State<Shell> createState() => _ShellState();
}

enum _PlaceKind { collection, artist, album, playlist }

/// One entry on the navigation stack.
class _Place {
  _Place.of(Collection this.collection)
    : kind = _PlaceKind.collection,
      artistId = null,
      albumId = null,
      playlist = null;
  _Place.artist(int this.artistId)
    : kind = _PlaceKind.artist,
      collection = null,
      albumId = null,
      playlist = null;
  _Place.album(int this.albumId)
    : kind = _PlaceKind.album,
      collection = null,
      artistId = null,
      playlist = null;
  _Place.playlist(Playlist this.playlist)
    : kind = _PlaceKind.playlist,
      collection = null,
      artistId = null,
      albumId = null;

  final _PlaceKind kind;
  final Collection? collection;
  final int? artistId;
  final int? albumId;
  final Playlist? playlist;
}

class _ShellState extends State<Shell> {
  List<Track> _tracks = const [];
  String? _error;
  bool _loading = true;
  int _tab = int.tryParse(Platform.environment['BULBA_TAB'] ?? '') ?? 0;
  Color _accent = C.signal;
  int? _accentTrack;

  /// A real stack: back always pops exactly one place, whatever mix of
  /// collections, artists and albums the listener walked through.
  final List<_Place> _stack = [
    if (int.tryParse(Platform.environment['BULBA_ARTIST'] ?? '') != null)
      _Place.artist(int.parse(Platform.environment['BULBA_ARTIST']!)),
    if (int.tryParse(Platform.environment['BULBA_ALBUM'] ?? '') != null)
      _Place.album(int.parse(Platform.environment['BULBA_ALBUM']!)),
  ];
  Collection? get _collection => _stack.isEmpty ? null : _stack.last.collection;
  int? get _artistId => _stack.isEmpty ? null : _stack.last.artistId;
  int? get _albumId => _stack.isEmpty ? null : _stack.last.albumId;

  /// `BULBA_PLAYLIST=12` opens that playlist on launch. Same review harness as
  /// BULBA_ALBUM: a screen that can only be reached by three taps is a screen
  /// that never gets looked at.
  void _openPlaylistFromEnv() {
    final id = int.tryParse(Platform.environment['BULBA_PLAYLIST'] ?? '');
    if (id == null) return;
    widget.api.playlists().then((list) {
      if (!mounted) return;
      for (final pl in list) {
        if (pl.id == id) {
          setState(() => _stack.add(_Place.playlist(pl)));
          return;
        }
      }
    }, onError: (_) {});
  }

  @override
  void initState() {
    super.initState();
    // `BULBA_CAST=1` opens the cast sheet on launch: discovery is the part that
    // cannot be unit-tested, and a screenshot of a device list is the only
    // evidence a desk can produce.
    if (Platform.environment['BULBA_CAST'] == '1') {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        showCastSheet(
          context,
          api: widget.api,
          player: widget.player,
          cast: widget.cast,
        );
      });
    }
    widget.player.addListener(_onPlayer);
    _load();
    widget.likes.load();
    _openPlaylistFromEnv();
    // Review harness for the duplicates screen: it lives three taps deep in the
    // settings, which means nobody would ever look at it.
    if (Platform.environment['BULBA_DUPLICATES'] == '1') {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (!mounted) return;
        Navigator.of(context).push(
          MaterialPageRoute<void>(
            builder: (_) => DuplicatesScreen(api: widget.api),
          ),
        );
      });
    }
  }

  @override
  void dispose() {
    widget.player.removeListener(_onPlayer);
    super.dispose();
  }

  bool _playerOpen = false;
  int? _lastTrackId;
  bool _lastPlaying = false;

  void _onPlayer() {
    final t = widget.player.current;
    if (t != null && t.id != _accentTrack) {
      _accentTrack = t.id;
      final url = widget.api.cover(t.albumId, t.hasCover);
      AccentCache.of(url).then((c) {
        if (mounted) setState(() => _accent = c);
      });
    }
    // The player notifies on every position tick. Rebuilding the whole shell
    // there repainted every screen dozens of times a second; only a new track
    // or a play/pause actually changes what this layer shows.
    final playing = widget.player.playing;
    if (t?.id != _lastTrackId || playing != _lastPlaying) {
      _lastTrackId = t?.id;
      _lastPlaying = playing;
      setState(() {});
    }
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
          _openCollection(
            Collection(
              title: mix.title,
              subtitle: mix.note,
              tracks: mix.tracks,
              mosaic: covers,
            ),
          );
        }
      }
      // `BULBA_PLAY=<id>` plays one specific track, which is how offline
      // playback is reviewed: download it, stop the server, press play.
      final playId = int.tryParse(Platform.environment['BULBA_PLAY'] ?? '');
      if (playId != null) {
        final target = tracks.where((t) => t.id == playId).toList();
        if (target.isNotEmpty) {
          await widget.player.play(target.first, queue: tracks);
          if (mounted) _openPlayer();
        }
      } else if (autoPlay && tracks.isNotEmpty) {
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

  void _openArtist(int id) => setState(() => _stack.add(_Place.artist(id)));

  void _openAlbum(int id) => setState(() => _stack.add(_Place.album(id)));

  void _openPlaylist(Playlist p) =>
      setState(() => _stack.add(_Place.playlist(p)));

  void _openCollection(Collection c) =>
      setState(() => _stack.add(_Place.of(c)));

  void _back() {
    if (_stack.isEmpty) return;
    setState(() => _stack.removeLast());
  }

  Future<void> _openAlbumById(int albumId, String title, String artist) async {
    try {
      final tracks = await widget.api.albumTracks(albumId, title);
      if (!mounted || tracks.isEmpty) return;
      _openCollection(
        Collection(title: title, subtitle: artist, tracks: tracks),
      );
    } catch (_) {}
  }

  Future<void> _playStatTrack(StatTrack st) async {
    try {
      final track = await widget.api.track(st.id);
      if (track == null || !mounted) return;
      await widget.player.play(track, queue: [track]);
      _openPlayer();
    } catch (_) {}
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

  int? _waveSeed;
  String? _waveSeedTitle;

  void _startFromTrack(Track t) {
    setState(() {
      _waveSeed = t.id;
      _waveSeedTitle = t.title;
      _tab = kWaveTab;
      _stack.clear();
    });
  }

  /// Everything that used to say «радио от этого трека» now seeds the wave:
  /// one endless stream that keeps adapting, instead of one frozen list.
  Future<void> _startRadio(Track seed) async {
    _startFromTrack(seed);
  }

  void _openPlayer() {
    if (widget.player.current == null) return;
    _playerOpen = true;
    Navigator.of(context)
        .push(
          PixelPageRoute<void>(
            builder: (_) => AccentScope(
              color: _accent,
              child: PlayerScreen(
                player: widget.player,
                api: widget.api,
                likes: widget.likes,
                cast: widget.cast,
                onRadio: _startRadio,
              ),
            ),
          ),
        )
        .whenComplete(() => _playerOpen = false);
  }

  /// True while a text field owns the keyboard. Shortcuts are checked before
  /// the focused widget sees the key, so without this guard typing a space in
  /// search would pause the track instead of entering a space.
  bool _typing() {
    final focus = FocusManager.instance.primaryFocus?.context;
    if (focus == null) return false;
    var typing = false;
    focus.visitAncestorElements((element) {
      if (element.widget is EditableText) {
        typing = true;
        return false;
      }
      return true;
    });
    return typing;
  }

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    final size = MediaQuery.sizeOf(context);
    final wide = size.width >= 900;

    void guard(VoidCallback action) {
      if (_typing()) return;
      action();
    }

    return PopScope(
      // Android's back gesture pops one place at a time; the app only leaves
      // when there is nowhere left to go.
      // Never let the system pop the root: back walks the app first and only
      // leaves when the listener is already on home.
      canPop: false,
      onPopInvokedWithResult: (didPop, _) {
        if (didPop) return;
        if (_playerOpen) {
          _playerOpen = false;
          Navigator.of(context).maybePop();
        } else if (_stack.isNotEmpty) {
          _back();
        } else if (_tab != 0) {
          _select(0);
        } else {
          SystemNavigator.pop();
        }
      },
      child: CallbackShortcuts(
        bindings: {
          const SingleActivator(LogicalKeyboardKey.space): () =>
              guard(widget.player.toggle),
          const SingleActivator(LogicalKeyboardKey.arrowRight): () =>
              guard(() => widget.player.skipBy(const Duration(seconds: 5))),
          const SingleActivator(LogicalKeyboardKey.arrowLeft): () =>
              guard(() => widget.player.skipBy(const Duration(seconds: -5))),
          const SingleActivator(
            LogicalKeyboardKey.arrowRight,
            shift: true,
          ): () =>
              guard(() => widget.player.skipBy(const Duration(seconds: 30))),
          const SingleActivator(
            LogicalKeyboardKey.arrowLeft,
            shift: true,
          ): () =>
              guard(() => widget.player.skipBy(const Duration(seconds: -30))),
          const SingleActivator(LogicalKeyboardKey.arrowUp): () =>
              guard(() => widget.player.setVolume(widget.player.volume + 0.05)),
          const SingleActivator(LogicalKeyboardKey.arrowDown): () =>
              guard(() => widget.player.setVolume(widget.player.volume - 0.05)),
          const SingleActivator(LogicalKeyboardKey.keyN): () =>
              guard(widget.player.next),
          const SingleActivator(LogicalKeyboardKey.keyP): () =>
              guard(widget.player.previous),
          const SingleActivator(LogicalKeyboardKey.keyL): () => guard(() {
            final t = widget.player.current;
            if (t != null) widget.likes.toggle(t);
          }),
          const SingleActivator(LogicalKeyboardKey.keyQ): () => guard(() {
            if (widget.player.queue.isEmpty) return;
            showQueue(
              context,
              player: widget.player,
              api: widget.api,
              likes: widget.likes,
            );
          }),
          const SingleActivator(LogicalKeyboardKey.keyF): () =>
              guard(_openPlayer),
          const SingleActivator(LogicalKeyboardKey.slash): () =>
              guard(() => setState(() => _tab = 1)),
        },
        child: AccentScope(
          color: _accent,
          child: Focus(autofocus: true, child: _scaffold(p, wide)),
        ),
      ),
    );
  }

  Widget _scaffold(Palette p, bool wide) {
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
        // Likes change rarely but touch every row: one rebuild of the body
        // keeps the hearts in lists honest the moment they are pressed.
        Expanded(
          child: ListenableBuilder(
            listenable: widget.likes,
            builder: (context, _) => _stacked(),
          ),
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
              Padding(
                padding: EdgeInsets.only(bottom: wide ? 0 : S.xs),
                child: PlayerBar(
                  player: widget.player,
                  api: widget.api,
                  likes: widget.likes,
                  onOpen: _openPlayer,
                  accent: _accent,
                  cast: widget.cast,
                  height: wide ? 68 : 60,
                  showVolume: wide,
                ),
              ),
            if (!wide)
              // An opaque strip under the dock: the capsule is translucent, and
              // list rows showing through it read as a rendering bug.
              Container(
                color: p.canvas,
                padding: const EdgeInsets.only(top: S.xs),
                child: FloatingNav(
                  index: _tab,
                  onSelect: _select,
                  accent: _accent,
                ),
              ),
          ],
        ),
      ),
    );
  }

  /// Tabs that have been visited at least once. A tab is built on first visit
  /// and kept alive afterwards, so coming back to the library does not refetch
  /// it, does not reset its scroll and does not replay its entrance: the shell
  /// remembering where you were is most of what "feels like an app" means.
  late final Set<int> _visited = {_tab};

  void _select(int i) => setState(() {
    _tab = i;
    _stack.clear();
    _visited.add(i);
  });

  Widget _settings() => SettingsScreen(
    profiles: widget.profiles,
    onProfileChanged: widget.onProfileChanged,
    api: widget.api,
    player: widget.player,
    prefs: widget.prefs,
    themeMode: widget.themeMode,
    likes: widget.likes,
    onRescan: _rescan,
    onOpenAlbum: _openAlbumById,
    onPlayTrack: _playStatTrack,
  );

  /// The detail stack wins over the tab body, but the tab bodies underneath stay
  /// alive: an artist opened from the library and popped must return to the
  /// library exactly as it was, not to a fresh fetch.
  Widget _stacked() {
    final tabs = IndexedStack(
      key: const ValueKey('tabs'),
      index: _tab,
      children: [
        for (var i = 0; i <= kSettingsTab; i++)
          if (_visited.contains(i)) _screenFor(i) else const SizedBox.shrink(),
      ],
    );
    if (_stack.isEmpty) return tabs;
    // The detail sits *above* the tabs rather than replacing them: opening an
    // album and going back must return to the library exactly as it was.
    return Stack(
      children: [
        tabs,
        Positioned.fill(
          child: ColoredBox(
            color: Palette.of(context).canvas,
            child: _screen(),
          ),
        ),
      ],
    );
  }

  Widget _screen() => _screenFor(_tab);

  Widget _screenFor(int tab) {
    final p = Palette.of(context);
    if (tab == kSettingsTab) return _settings();
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
            Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                BracketButton('retry', onTap: _load),
                const SizedBox(width: S.md),
                BracketButton('settings', onTap: () => _select(kSettingsTab)),
              ],
            ),
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
    if (_artistId != null) {
      return ArtistScreen(
        api: widget.api,
        player: widget.player,
        likes: widget.likes,
        artistId: _artistId!,
        onAlbum: _openAlbum,
        onRadio: _startRadio,
        onBack: _back,
      );
    }
    final playlist = _stack.isEmpty ? null : _stack.last.playlist;
    if (playlist != null) {
      return PlaylistScreen(
        api: widget.api,
        player: widget.player,
        likes: widget.likes,
        playlist: playlist,
        onRadio: _startRadio,
        onBack: _back,
      );
    }
    if (_albumId != null) {
      return AlbumScreen(
        api: widget.api,
        player: widget.player,
        likes: widget.likes,
        albumId: _albumId!,
        onArtist: _openArtist,
        onRadio: _startRadio,
        onBack: _back,
      );
    }
    if (collection != null) {
      return CollectionView(
        collection: collection,
        api: widget.api,
        player: widget.player,
        onRadio: _startRadio,
        onBack: _back,
        likes: widget.likes,
        accent: _accent,
      );
    }
    switch (tab) {
      case 0:
        return HomeScreen(
          tracks: _tracks,
          api: widget.api,
          player: widget.player,
          onRadio: _startRadio,
          onRescan: _rescan,
          accent: _accent,
          onOpen: _openCollection,
          onOpenAlbum: _openAlbum,
          onOpenLibrary: () => _select(2),
        );
      case kWaveTab:
        return WaveScreen(
          api: widget.api,
          player: widget.player,
          likes: widget.likes,
          accent: _accent,
          seed: _waveSeed,
          seedTitle: _waveSeedTitle,
        );
      case kSearchTab:
        return SearchScreen(
          api: widget.api,
          player: widget.player,
          onRadio: _startRadio,
          onOpen: _openCollection,
          likes: widget.likes,
        );
      case kLibraryTab:
        return LibraryScreen(
          tracks: _tracks,
          api: widget.api,
          player: widget.player,
          likes: widget.likes,
          onArtist: _openArtist,
          onAlbum: _openAlbum,
          onRadio: _startRadio,
        );
      case 4:
        return LikedScreen(
          api: widget.api,
          player: widget.player,
          likes: widget.likes,
          onRadio: _startRadio,
          onOpenPlaylist: _openPlaylist,
          onOpenCollection: (tracks, title) => _openCollection(
            Collection(
              title: title,
              subtitle: '${tracks.length} треков',
              tracks: tracks,
            ),
          ),
        );
      default:
        return _settings();
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
