import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:media_kit/media_kit.dart' show MediaKit;

import 'api/client.dart';
import 'design/tokens.dart';
import 'player/background.dart';
import 'player/controller.dart';
import 'player/likes.dart';
import 'screens/auth.dart';
import 'screens/onboarding.dart';
import 'state/prefs.dart';
import 'shell.dart';

/// Set with `--dart-define=SHOT=/path.png` to dump the first frame to a file.
const _shotPath = String.fromEnvironment('SHOT');

/// `light`, `dark` or `system` — lets the design be reviewed in both palettes
/// without touching the OS setting.
const _forceTheme = String.fromEnvironment('THEME');

void main() {
  WidgetsFlutterBinding.ensureInitialized();
  MediaKit.ensureInitialized();
  runApp(const BulbaApp());
}

class BulbaApp extends StatefulWidget {
  const BulbaApp({super.key});

  @override
  State<BulbaApp> createState() => _BulbaAppState();
}

class _BulbaAppState extends State<BulbaApp> {
  final GlobalKey _shotKey = GlobalKey();

  /// Runtime override wins over the compile-time one so the palette can be
  /// reviewed without a rebuild: `BULBA_THEME=light ./bulba_music`.
  String get _themeName => Platform.environment['BULBA_THEME'] ?? _forceTheme;

  late final ValueNotifier<ThemeMode> themeMode = ValueNotifier(
    switch (_themeName) {
      'light' => ThemeMode.light,
      'dark' => ThemeMode.dark,
      _ => ThemeMode.system,
    },
  );

  late final Api api = Api(
    const String.fromEnvironment(
      'BULBA_SERVER',
      // Desktop talks to its own server over loopback (and is therefore the
      // owner without a phrase). The phone build passes the tailnet address.
      defaultValue: 'http://127.0.0.1:8422',
    ),
  );
  late final PlayerController player = PlayerController(api);
  late final LikesStore likes = LikesStore(api);
  Prefs? _prefs;
  bool _ready = false;
  bool _onboarding = false;
  bool _needsAuth = false;

  @override
  void initState() {
    super.initState();
    _boot();
    if (_shotPath.isNotEmpty) _captureAfterFirstFrame();
  }

  Future<void> _boot() async {
    final prefs = await Prefs.load();
    themeMode.value = Prefs.modeFrom(prefs.themeMode);
    api.token = prefs.token;
    player.crossfade = prefs.crossfade;
    player.quality = prefs.quality;
    player.replayGain = prefs.replayGain;
    await player.setEq(prefs.eq);
    final savedServer = prefs.server;
    if (savedServer != null && savedServer.isNotEmpty) api.base = savedServer;
    player.setVolume(prefs.volume);
    // Android suspends the process without a foreground service, so the audio
    // dies with the window. Start it before anything can play.
    await startBackgroundAudio(player, api);
    final skipOnboarding =
        const bool.fromEnvironment('SHOT_ONBOARD') ||
        Platform.environment['BULBA_SKIP_ONBOARDING'] == '1';
    final auth = await api.authStatus();
    if (!mounted) return;
    setState(() {
      _prefs = prefs;
      _needsAuth = auth == 'needs_auth';
      _onboarding = !prefs.onboardingSeen && !skipOnboarding && !_needsAuth;
      _ready = true;
    });
  }

  Future<void> _captureAfterFirstFrame() async {
    await Future<void>.delayed(const Duration(seconds: 4));
    final boundary =
        _shotKey.currentContext?.findRenderObject() as RenderRepaintBoundary?;
    if (boundary == null) return;
    final image = await boundary.toImage(pixelRatio: 2);
    final data = await image.toByteData(format: ui.ImageByteFormat.png);
    if (data == null) return;
    File(_shotPath).writeAsBytesSync(data.buffer.asUint8List());
  }

  @override
  void dispose() {
    themeMode.dispose();
    player.dispose();
    super.dispose();
  }

  ThemeData _theme(Palette p, Brightness brightness) => ThemeData(
    brightness: brightness,
    scaffoldBackgroundColor: p.canvas,
    fontFamily: 'JetBrainsMono',
    splashFactory: NoSplash.splashFactory,
    highlightColor: Colors.transparent,
    colorScheme: ColorScheme(
      brightness: brightness,
      surface: p.canvas,
      primary: p.ink,
      secondary: C.signal,
      onSurface: p.ink,
      onPrimary: p.canvas,
      onSecondary: p.canvas,
      error: C.danger,
      onError: p.canvas,
    ),
    // Everything in this app is rounded; Material's defaults are not, and a
    // square grey flash on a rounded row is exactly the thing that looked
    // broken on a phone.
    listTileTheme: ListTileThemeData(
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(10)),
      selectedTileColor: Colors.transparent,
    ),
    dialogTheme: DialogThemeData(
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(16)),
    ),
    popupMenuTheme: PopupMenuThemeData(
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
    ),
    snackBarTheme: SnackBarThemeData(
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
      behavior: SnackBarBehavior.floating,
    ),
    textSelectionTheme: TextSelectionThemeData(
      cursorColor: C.signal,
      selectionColor: C.signal.withValues(alpha: 0.2),
    ),
  );

  @override
  Widget build(BuildContext context) {
    debugPrint(
      'theme name="$_themeName" env="${Platform.environment['BULBA_THEME']}"',
    );
    return RepaintBoundary(
      key: _shotKey,
      child: ValueListenableBuilder<ThemeMode>(
        valueListenable: themeMode,
        builder: (context, mode, _) => MaterialApp(
          builder: (context, child) =>
              PreviewSize(child: child ?? const SizedBox()),
          title: 'bulba music',
          debugShowCheckedModeBanner: false,
          theme: _theme(Palette.light, Brightness.light),
          darkTheme: _theme(Palette.inkNight, Brightness.dark),
          themeMode: mode,
          home: !_ready
              ? const ColoredBox(color: Color(0xFF141210))
              : _needsAuth
              ? AuthScreen(
                  api: api,
                  player: player,
                  likes: likes,
                  onSignedIn: (token) async {
                    await _prefs?.setToken(token);
                    if (mounted) setState(() => _needsAuth = false);
                  },
                )
              : _onboarding
              ? Onboarding(
                  api: api,
                  initialTheme: themeMode.value,
                  onTheme: (m) {
                    themeMode.value = m;
                    _prefs?.setThemeMode(switch (m) {
                      ThemeMode.light => 'light',
                      ThemeMode.system => 'system',
                      _ => 'dark',
                    });
                  },
                  onDone: () async {
                    await _prefs?.setOnboardingSeen();
                    setState(() => _onboarding = false);
                  },
                )
              : Shell(
                  api: api,
                  player: player,
                  likes: likes,
                  prefs: _prefs!,
                  themeMode: themeMode,
                ),
        ),
      ),
    );
  }
}

/// BULBA_WINDOW=390x844 makes the app lay itself out for a phone while the
/// desktop window stays whatever it is. It only exists so narrow layouts can be
/// reviewed with the same screenshot harness as wide ones.
class PreviewSize extends StatelessWidget {
  const PreviewSize({super.key, required this.child});
  final Widget child;

  static Size? get size {
    final raw = Platform.environment['BULBA_WINDOW'];
    if (raw == null) return null;
    final parts = raw.split('x');
    if (parts.length != 2) return null;
    final w = double.tryParse(parts[0]);
    final h = double.tryParse(parts[1]);
    if (w == null || h == null || w < 200 || h < 200) return null;
    return Size(w, h);
  }

  @override
  Widget build(BuildContext context) {
    final target = size;
    if (target == null) return child;
    return Align(
      alignment: Alignment.center,
      child: SizedBox(
        width: target.width,
        height: target.height,
        child: MediaQuery(
          data: MediaQuery.of(context).copyWith(size: target),
          child: ClipRect(child: child),
        ),
      ),
    );
  }
}
