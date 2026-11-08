import 'dart:async';
import 'dart:io';
import 'dart:math' as math;
import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';

import 'tokens.dart';

/// The room: the current cover, blurred until it is only colour, with a scrim,
/// a vignette and a film of grain. Everything else in the app floats on it.
///
/// On the phone this is real GPU work (`ImageFiltered`), not a pre-blurred
/// bitmap: the cover arrives at 512 px and the blur runs on a texture the
/// compositor already has.
class RoomBackdrop extends StatelessWidget {
  const RoomBackdrop({super.key, required this.artUrl, required this.palette});

  final String? artUrl;
  final Palette palette;

  @override
  Widget build(BuildContext context) {
    final scrimTop = palette.canvas.withValues(alpha: 0.04);
    final scrimBottom = palette.canvas.withValues(alpha: 0.62);
    final url = artUrl;
    return RepaintBoundary(
      child: Stack(
        fit: StackFit.expand,
        children: [
          // A phone has no desktop behind it: the room's base is the cover's
          // own darkest shade, opaque, so nothing depends on what is behind the
          // window.
          ColoredBox(color: palette.room),
          if (url != null)
            Positioned.fill(
              child: TweenAnimationBuilder<double>(
                key: ValueKey(url),
                tween: Tween(begin: 0, end: 1),
                duration: const Duration(milliseconds: 700),
                curve: Curves.easeOutCubic,
                builder: (context, t, child) =>
                    Opacity(opacity: t * 0.85, child: child),
                child: ImageFiltered(
                  imageFilter: ui.ImageFilter.blur(sigmaX: 58, sigmaY: 58),
                  child: ColorFiltered(
                    colorFilter: const ColorFilter.matrix(_saturate),
                    child: DecoratedBox(
                      decoration: BoxDecoration(
                        image: DecorationImage(
                          image: _provider(url),
                          fit: BoxFit.cover,
                          onError: (e, st) {
                            // ignore: avoid_print
                            print('[room] cover failed: $e');
                          },
                        ),
                      ),
                    ),
                  ),
                ),
              ),
            ),
          DecoratedBox(
            decoration: BoxDecoration(
              gradient: LinearGradient(
                begin: Alignment.topCenter,
                end: Alignment.bottomCenter,
                colors: [scrimTop, scrimBottom],
              ),
            ),
          ),
          DecoratedBox(
            decoration: BoxDecoration(
              gradient: RadialGradient(
                radius: 1.15,
                colors: [
                  Colors.transparent,
                  Colors.black.withValues(alpha: palette.dark ? 0.34 : 0.14),
                ],
              ),
            ),
          ),
          const _Grain(opacity: 0.05),
        ],
      ),
    );
  }
}

ImageProvider _provider(String url) {
  if (url.startsWith('file://')) {
    return FileImage(File(Uri.parse(url).toFilePath()));
  }
  return NetworkImage(url);
}

/// 1.62× saturation, the same push the desktop applies to its backdrop.
const List<double> _saturate = <double>[
  1.62, -0.31, -0.31, 0, 0, //
  -0.31, 1.62, -0.31, 0, 0, //
  -0.31, -0.31, 1.62, 0, 0, //
  0, 0, 0, 1, 0,
];

/// A 96×96 tile of noise, drawn once into an image and repeated. The grain is
/// what separates glass from a flat gradient: without it every large surface
/// bands on an OLED panel.
class _Grain extends StatefulWidget {
  const _Grain({required this.opacity});

  final double opacity;

  @override
  State<_Grain> createState() => _GrainState();
}

class _GrainState extends State<_Grain> {
  static ui.Image? _shared;
  ui.Image? _image;

  @override
  void initState() {
    super.initState();
    _image = _shared;
    if (_image == null) _build();
  }

  Future<void> _build() async {
    const n = 96;
    final random = math.Random(7);
    final bytes = Uint8List(n * n * 4);
    for (var i = 0; i < n * n; i++) {
      final v = random.nextInt(255);
      bytes[i * 4] = v;
      bytes[i * 4 + 1] = v;
      bytes[i * 4 + 2] = v;
      bytes[i * 4 + 3] = random.nextInt(110);
    }
    final done = Completer<ui.Image>();
    ui.decodeImageFromPixels(
      bytes,
      n,
      n,
      ui.PixelFormat.rgba8888,
      done.complete,
    );
    final image = await done.future;
    _shared = image;
    if (mounted) setState(() => _image = image);
  }

  @override
  Widget build(BuildContext context) {
    final image = _image;
    if (image == null) return const SizedBox.shrink();
    return IgnorePointer(
      child: CustomPaint(
        painter: _GrainPainter(image, widget.opacity),
        size: Size.infinite,
      ),
    );
  }
}

class _GrainPainter extends CustomPainter {
  _GrainPainter(this.image, this.opacity);

  final ui.Image image;
  final double opacity;

  @override
  void paint(Canvas canvas, Size size) {
    final shader = ui.ImageShader(
      image,
      TileMode.repeated,
      TileMode.repeated,
      Matrix4.identity().storage,
    );
    canvas.drawRect(
      Offset.zero & size,
      Paint()
        ..shader = shader
        ..blendMode = BlendMode.softLight
        ..color = Colors.white.withValues(alpha: opacity),
    );
  }

  @override
  bool shouldRepaint(_GrainPainter old) =>
      old.opacity != opacity || old.image != image;
}
