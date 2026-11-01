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

  static Palette of(BuildContext context) =>
      Theme.of(context).brightness == Brightness.dark ? inkNight : light;
}

abstract final class C {
  /// The cover's hue, used in exactly two places: played progress and the
  /// currently playing marker.
  static const signal = Color(0xFFC2410C);
  static const danger = Color(0xFFFF3B30);
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
TextStyle mono(
  double size,
  double weight,
  Color color, {
  double height = 1.55,
  double? letterSpacing,
  TextDecoration? decoration,
}) => TextStyle(
  fontFamily: 'JetBrainsMono',
  fontSize: size,
  height: height,
  letterSpacing: letterSpacing,
  color: color,
  decoration: decoration,
  fontVariations: [FontVariation('wght', weight)],
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
