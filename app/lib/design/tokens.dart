import 'package:flutter/material.dart';

/// Two palettes, one system: paper for daylight, ink for the night. The player
/// room is always the darkest surface in either of them.
class Palette {
  const Palette({
    required this.dark,
    required this.canvas,
    required this.ink,
    required this.inkDeep,
    required this.body,
    required this.mute,
    required this.stone,
    required this.ash,
    required this.soft,
    required this.card,
    required this.hairline,
    required this.hairlineStrong,
    required this.room,
    required this.roomElevated,
    required this.onRoom,
    required this.onRoomMute,
  });

  final bool dark;
  final Color canvas;
  final Color ink;
  final Color inkDeep;
  final Color body;
  final Color mute;
  final Color stone;
  final Color ash;
  final Color soft;
  final Color card;
  final Color hairline;
  final Color hairlineStrong;
  final Color room;
  final Color roomElevated;
  final Color onRoom;
  final Color onRoomMute;

  static const light = Palette(
    dark: false,
    canvas: Color(0xFFFDFCFC),
    ink: Color(0xFF201D1D),
    inkDeep: Color(0xFF0F0000),
    body: Color(0xFF424245),
    mute: Color(0xFF646262),
    stone: Color(0xFF6E6E73),
    ash: Color(0xFF9A9898),
    soft: Color(0xFFF8F7F7),
    card: Color(0xFFF1EEEE),
    hairline: Color(0x1F0F0000),
    hairlineStrong: Color(0xFF646262),
    room: Color(0xFF201D1D),
    roomElevated: Color(0xFF302C2C),
    onRoom: Color(0xFFFDFCFC),
    onRoomMute: Color(0xFF9A9898),
  );

  static const inkNight = Palette(
    dark: true,
    canvas: Color(0xFF161414),
    ink: Color(0xFFFDFCFC),
    inkDeep: Color(0xFFFFFFFF),
    body: Color(0xFFD9D4D4),
    mute: Color(0xFF9A9898),
    stone: Color(0xFF85838A),
    ash: Color(0xFF6E6E73),
    soft: Color(0xFF1E1B1B),
    card: Color(0xFF2A2525),
    hairline: Color(0x1FFDFCFC),
    hairlineStrong: Color(0xFF6E6E73),
    room: Color(0xFF0D0B0B),
    roomElevated: Color(0xFF241F1F),
    onRoom: Color(0xFFFDFCFC),
    onRoomMute: Color(0xFF9A9898),
  );

  /// A room lit by the record: the same rule the GPUI desktop uses. A bright
  /// cover opens a bright room, a dark one stays night, and every surface is
  /// translucent because there is a blurred cover behind it.
  factory Palette.fromArt(Color accent) {
    final hsl = HSLColor.fromColor(accent);
    final h = hsl.hue;
    final light = accent.computeLuminance() > 0.42;
    Color a(double alpha, double sat, double lum) => HSLColor.fromAHSL(
      alpha,
      h,
      sat.clamp(0.0, 1.0),
      lum.clamp(0.0, 1.0),
    ).toColor();
    if (light) {
      return Palette(
        dark: false,
        canvas: a(1.0, hsl.saturation * 0.16, 0.94),
        ink: a(1, 0.22, 0.10),
        inkDeep: const Color(0xFF0F0000),
        body: a(1, 0.14, 0.22),
        mute: a(1, 0.10, 0.38),
        stone: a(1, 0.08, 0.46),
        ash: a(1, 0.07, 0.56),
        soft: a(0.55, 0.20, 1.0),
        card: a(0.66, 0.30, 1.0),
        hairline: a(0.16, 0.35, 0.10),
        hairlineStrong: a(0.40, 0.20, 0.25),
        room: a(0.92, 0.25, 0.12),
        roomElevated: a(0.94, 0.20, 0.20),
        onRoom: const Color(0xFFFDFCFC),
        onRoomMute: a(1, 0.06, 0.70),
      );
    }
    return Palette(
      dark: true,
      canvas: a(1.0, hsl.saturation * 0.18, 0.045),
      ink: a(1, 0.03, 0.975),
      inkDeep: const Color(0xFFFFFFFF),
      body: a(1, 0.04, 0.86),
      mute: a(1, 0.04, 0.68),
      stone: a(1, 0.03, 0.58),
      ash: a(1, 0.03, 0.50),
      soft: a(0.30, 0.20, 1.0),
      card: a(0.08, 0.22, 1.0),
      hairline: a(0.11, 0.20, 1.0),
      hairlineStrong: a(0.35, 0.10, 0.75),
      room: a(0.86, 0.25, 0.03),
      roomElevated: a(0.92, 0.20, 0.10),
      onRoom: const Color(0xFFFDFCFC),
      onRoomMute: a(1, 0.03, 0.62),
    );
  }

  static Palette of(BuildContext context) {
    final room = context.dependOnInheritedWidgetOfExactType<RoomScope>();
    if (room != null) return room.palette;
    return Theme.of(context).brightness == Brightness.dark ? inkNight : light;
  }
}

/// The room the whole app is drawn in: the palette lifted from the current
/// cover, and the cover itself so a backdrop can be painted behind everything.
class RoomScope extends InheritedWidget {
  const RoomScope({
    super.key,
    required this.palette,
    required this.artUrl,
    required super.child,
  });

  final Palette palette;
  final String? artUrl;

  static RoomScope? maybeOf(BuildContext context) =>
      context.dependOnInheritedWidgetOfExactType<RoomScope>();

  @override
  bool updateShouldNotify(RoomScope old) =>
      old.palette != palette || old.artUrl != artUrl;
}

abstract final class C {
  /// Fallback before a cover is known, and the value used by widgets built
  /// without an ancestor [AccentScope] (tests, dialogs opened from nowhere).
  static const signal = Color(0xFFC2410C);
  static const danger = Color(0xFFFF3B30);
}

/// The colour of the current cover, handed down the tree.
///
/// Everything that used to hardcode orange — progress, the playing marker,
/// sliders, hearts, links, buttons — reads from here, so a track change moves
/// the whole interface, not half of it.
class AccentScope extends InheritedWidget {
  const AccentScope({super.key, required this.color, required super.child});

  final Color color;

  static Color of(BuildContext context) {
    final scope = context.dependOnInheritedWidgetOfExactType<AccentScope>();
    return scope?.color ?? C.signal;
  }

  @override
  bool updateShouldNotify(AccentScope old) => old.color != color;
}

abstract final class S {
  static const xxs = 2.0;
  static const xs = 4.0;
  static const sm = 8.0;
  static const md = 12.0;
  static const lg = 16.0;
  static const xl = 24.0;
  static const xxl = 32.0;
  static const section = 64.0;
}

abstract final class R {
  static const art = 2.0;
  static const sm = 4.0;
}

/// One monospace face carries every role. Weight is a variable axis, so it is
/// expressed through [FontVariation] rather than a family per weight.
/// Geist carries every role now, the same face the desktop draws with. The
/// function keeps its old name so the twenty screens that call `mono()` do not
/// need to care what the family is today.
TextStyle mono(
  double size,
  double weight,
  Color color, {
  double height = 1.45,
  double? letterSpacing,
  TextDecoration? decoration,
}) => TextStyle(
  fontFamily: 'Geist',
  fontSize: size,
  height: height,
  letterSpacing: letterSpacing,
  color: color,
  decoration: decoration,
  fontWeight: switch (weight) {
    >= 700 => FontWeight.w700,
    >= 600 => FontWeight.w600,
    >= 500 => FontWeight.w500,
    _ => FontWeight.w400,
  },
);

abstract final class T {
  static TextStyle hero(Color c) => mono(44, 700, c, height: 1.1);
  static TextStyle displayL(Color c) => mono(32, 700, c, height: 1.2);
  static TextStyle displayM(Color c) => mono(24, 700, c, height: 1.25);
  static TextStyle heading(Color c) => mono(16, 700, c);
  static TextStyle body(Color c) => mono(15, 400, c, height: 1.6);
  static TextStyle bodyStrong(Color c) => mono(15, 500, c, height: 1.6);
  static TextStyle label(Color c) => mono(13, 500, c);
  static TextStyle caption(Color c) => mono(12, 400, c, height: 1.6);
  static TextStyle micro(Color c) => mono(11, 500, c, height: 1.4);
}

String mmss(Duration d) {
  final m = d.inMinutes.remainder(60).toString().padLeft(2, '0');
  final s = d.inSeconds.remainder(60).toString().padLeft(2, '0');
  return d.inHours > 0 ? '${d.inHours}:$m:$s' : '$m:$s';
}

/// Russian plural for counts: 1 трек, 2 трека, 5 треков.
String plural(
  int n, {
  String one = 'трек',
  String few = 'трека',
  String many = 'треков',
}) {
  final mod100 = n % 100;
  final mod10 = n % 10;
  if (mod100 >= 11 && mod100 <= 14) return many;
  if (mod10 == 1) return one;
  if (mod10 >= 2 && mod10 <= 4) return few;
  return many;
}
