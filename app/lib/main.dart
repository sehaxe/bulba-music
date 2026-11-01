import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:media_kit/media_kit.dart' show MediaKit;

import 'api/client.dart';
import 'design/tokens.dart';
import 'player/controller.dart';
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
      defaultValue: 'http://127.0.0.1:8422',
    ),
  );
  late final PlayerController player = PlayerController(api);

  @override
  void initState() {
    super.initState();
    if (_shotPath.isNotEmpty) _captureAfterFirstFrame();
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
          title: 'bulba music',
          debugShowCheckedModeBanner: false,
          theme: _theme(Palette.light, Brightness.light),
          darkTheme: _theme(Palette.inkNight, Brightness.dark),
          themeMode: mode,
          home: Shell(api: api, player: player, themeMode: themeMode),
        ),
      ),
    );
  }
}
