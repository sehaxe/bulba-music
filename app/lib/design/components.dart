import 'dart:math' as math;

import 'package:flutter/scheduler.dart';
import 'package:flutter/widgets.dart';

import 'tokens.dart';

/// A 1px rule. The only separator the system allows.
class Hr extends StatelessWidget {
  const Hr({super.key, this.color, this.indent = 0});
  final Color? color;
  final double indent;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    return Padding(
      padding: EdgeInsets.only(left: indent),
      child: Container(height: 1, color: color ?? p.hairline),
    );
  }
}

/// `[label]` — the system's only button shape. No fill, no shadow, no icon.
class BracketButton extends StatefulWidget {
  const BracketButton(
    this.label, {
    super.key,
    this.onTap,
    this.active = false,
    this.size = 13,
    this.color,
    this.activeColor = C.signal,
    this.enabled = true,
  });

  final String label;
  final VoidCallback? onTap;
  final bool active;
  final double size;
  final Color? color;
  final Color activeColor;
  final bool enabled;

  @override
  State<BracketButton> createState() => _BracketButtonState();
}

class _BracketButtonState extends State<BracketButton> {
  bool _hover = false;
  bool _down = false;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    final base = widget.color ?? p.ink;
    final Color c = !widget.enabled
        ? p.ash
        : widget.active
        ? widget.activeColor
        : _hover
        ? p.inkDeep
        : base;
    final brackets = _down ? ['⟨', '⟩'] : ['[', ']'];
    return MouseRegion(
      cursor: widget.enabled
          ? SystemMouseCursors.click
          : SystemMouseCursors.basic,
      onEnter: (_) => setState(() => _hover = true),
      onExit: (_) => setState(() => _hover = _down = false),
      child: GestureDetector(
        onTapDown: widget.enabled ? (_) => setState(() => _down = true) : null,
        onTapCancel: () => setState(() => _down = false),
        onTapUp: widget.enabled ? (_) => setState(() => _down = false) : null,
        onTap: widget.enabled ? widget.onTap : null,
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 90),
          curve: Curves.easeOut,
          padding: const EdgeInsets.symmetric(horizontal: 2, vertical: 6),
          child: Text(
            '${brackets[0]}${widget.label}${brackets[1]}',
            style: mono(widget.size, 500, c, height: 1.2),
          ),
        ),
      ),
    );
  }
}

/// Progress and volume are glyph rows, not sliders: `████░░░░`.
class AsciiBar extends StatelessWidget {
  const AsciiBar({
    super.key,
    required this.value,
    this.played,
    this.rest,
    this.size = 13,
    this.onSeek,
  });

  final double value;
  final Color? played;
  final Color? rest;
  final double size;
  final ValueChanged<double>? onSeek;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    final playedColor = played ?? p.ink;
    final restColor = rest ?? p.ash;
    final style = mono(size, 400, playedColor, height: 1.0);
    return LayoutBuilder(
      builder: (context, box) {
        final painter = TextPainter(
          text: TextSpan(text: '█', style: style),
          textDirection: TextDirection.ltr,
        )..layout();
        final cw = math.max(1.0, painter.width);
        final cells = math.max(1, (box.maxWidth / cw).floor());
        final filled = (cells * value.clamp(0.0, 1.0)).floor();
        final bar = '█' * filled + '░' * (cells - filled);
        final text = Text(
          bar,
          maxLines: 1,
          softWrap: false,
          style: style.copyWith(
            foreground: Paint()
              ..shader = LinearGradient(
                colors: [playedColor, playedColor, restColor, restColor],
                stops: [0, filled / cells, filled / cells, 1],
              ).createShader(Rect.fromLTWH(0, 0, cw * cells, size)),
          ),
        );
        if (onSeek == null) return text;
        return GestureDetector(
          behavior: HitTestBehavior.opaque,
          onTapDown: (d) => onSeek!(d.localPosition.dx / (cw * cells)),
          onHorizontalDragUpdate: (d) =>
              onSeek!(d.localPosition.dx / (cw * cells)),
          child: text,
        );
      },
    );
  }
}

/// Missing art: a hatched square, never a broken image icon.
class Hatch extends StatelessWidget {
  const Hatch({
    super.key,
    this.size = 44,
    this.radius = R.art,
    this.dark = false,
  });
  final double size;
  final double radius;
  final bool dark;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    final bg = dark ? p.roomElevated : p.card;
    return ClipRRect(
      borderRadius: BorderRadius.circular(radius),
      child: Container(
        width: size,
        height: size,
        decoration: BoxDecoration(
          color: bg,
          border: Border.all(color: dark ? p.roomElevated : p.hairline),
          borderRadius: BorderRadius.circular(radius),
        ),
        child: CustomPaint(painter: _HatchPainter(dark ? p.onRoomMute : p.ash)),
      ),
    );
  }
}

class _HatchPainter extends CustomPainter {
  _HatchPainter(this.color);
  final Color color;

  @override
  void paint(Canvas canvas, Size size) {
    final p = Paint()
      ..color = color.withValues(alpha: 0.25)
      ..strokeWidth = 1;
    const step = 6.0;
    for (double x = -size.height; x < size.width; x += step) {
      canvas.drawLine(Offset(x, size.height), Offset(x + size.height, 0), p);
    }
  }

  @override
  bool shouldRepaint(_HatchPainter old) => old.color != color;
}

/// Cover art with a Bayer-dither reveal: the image resolves through a 4×4
/// ordered mask instead of fading in. Missing art falls back to [Hatch].
class CoverArt extends StatefulWidget {
  const CoverArt({
    super.key,
    this.url,
    required this.size,
    this.radius = R.art,
    this.animate = true,
    this.border = true,
  });

  final String? url;
  final double size;
  final double radius;
  final bool animate;
  final bool border;

  @override
  State<CoverArt> createState() => _CoverArtState();
}

class _CoverArtState extends State<CoverArt>
    with SingleTickerProviderStateMixin {
  late final AnimationController _c = AnimationController(
    vsync: this,
    duration: const Duration(milliseconds: 320),
  );

  @override
  void initState() {
    super.initState();
    if (widget.animate) {
      _c.forward();
    } else {
      _c.value = 1;
    }
  }

  @override
  void didUpdateWidget(CoverArt old) {
    super.didUpdateWidget(old);
    if (old.url != widget.url && widget.animate) {
      _c.forward(from: 0);
    }
  }

  @override
  void dispose() {
    _c.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    final placeholder = Hatch(size: widget.size, radius: widget.radius);
    final url = widget.url;
    final image = url == null
        ? placeholder
        : Image.network(
            url,
            width: widget.size,
            height: widget.size,
            // Decode at display size: a 1280px cover drawn at 40px otherwise
            // costs a full-size texture per row.
            cacheWidth: (widget.size * MediaQuery.devicePixelRatioOf(context))
                .round(),
            fit: BoxFit.cover,
            gaplessPlayback: true,
            errorBuilder: (_, _, _) => placeholder,
            frameBuilder: (context, child, frame, wasSync) {
              if (wasSync || frame != null) return child;
              return placeholder;
            },
          );

    return ClipRRect(
      borderRadius: BorderRadius.circular(widget.radius),
      child: Container(
        width: widget.size,
        height: widget.size,
        decoration: BoxDecoration(
          border: widget.border ? Border.all(color: p.hairline) : null,
          color: p.card,
        ),
        child: AnimatedBuilder(
          animation: _c,
          builder: (context, child) =>
              ClipPath(clipper: BayerClipper(_c.value), child: child),
          child: image,
        ),
      ),
    );
  }
}

class BayerClipper extends CustomClipper<Path> {
  BayerClipper(this.t, {this.cells = 18, this.radial = false});
  final double t;
  final int cells;

  /// Reveal from the centre outward: ordered noise plus a radial bias.
  final bool radial;

  static const _bayer = <List<int>>[
    [0, 8, 2, 10],
    [12, 4, 14, 6],
    [3, 11, 1, 9],
    [15, 7, 13, 5],
  ];

  @override
  Path getClip(Size size) {
    final path = Path();
    final n = cells;
    final cw = size.width / n;
    final ch = size.height / n;
    for (var y = 0; y < n; y++) {
      for (var x = 0; x < n; x++) {
        var threshold = (_bayer[y % 4][x % 4] + 0.5) / 16;
        if (radial) {
          final dx = (x + 0.5) / n - 0.5;
          final dy = (y + 0.5) / n - 0.5;
          final d = math.sqrt(dx * dx + dy * dy) / 0.71;
          threshold = threshold * 0.55 + d * 0.45;
        }
        if (threshold <= t) {
          path.addRect(Rect.fromLTWH(x * cw, y * ch, cw + 0.5, ch + 0.5));
        }
      }
    }
    return path;
  }

  @override
  bool shouldReclip(BayerClipper old) => old.t != t || old.cells != cells;
}

/// Terminal-redraw title: characters resolve left to right out of random
/// glyphs, the way a TUI repaints a line.
class ScrambleText extends StatefulWidget {
  const ScrambleText(
    this.text, {
    super.key,
    required this.style,
    this.duration = const Duration(milliseconds: 260),
  });

  final String text;
  final TextStyle style;
  final Duration duration;

  @override
  State<ScrambleText> createState() => _ScrambleTextState();
}

class _ScrambleTextState extends State<ScrambleText>
    with SingleTickerProviderStateMixin {
  late final AnimationController _c = AnimationController(
    vsync: this,
    duration: widget.duration,
  )..forward();

  static const _glyphs = r'▚▞░▒#%&@$*+=<>/\|_~^0123456789';

  @override
  void didUpdateWidget(ScrambleText old) {
    super.didUpdateWidget(old);
    if (old.text != widget.text) _c.forward(from: 0);
  }

  @override
  void dispose() {
    _c.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: _c,
      builder: (context, _) {
        final t = _c.value;
        if (t >= 1) return Text(widget.text, style: widget.style);
        final chars = widget.text.split('');
        final resolved = (t * chars.length).floor();
        final out = StringBuffer();
        for (var i = 0; i < chars.length; i++) {
          if (i < resolved || chars[i] == ' ') {
            out.write(chars[i]);
          } else {
            final seed = (i * 31 + (t * 120).floor()) % _glyphs.length;
            out.write(_glyphs[seed]);
          }
        }
        return Text(out.toString(), style: widget.style);
      },
    );
  }
}

/// Staggered entrance for list rows: 24ms apart, capped at eight so long
/// lists do not feel like a loading spinner.
class StaggerIn extends StatefulWidget {
  const StaggerIn({super.key, required this.index, required this.child});
  final int index;
  final Widget child;

  @override
  State<StaggerIn> createState() => _StaggerInState();
}

class _StaggerInState extends State<StaggerIn>
    with SingleTickerProviderStateMixin {
  late final AnimationController _c = AnimationController(
    vsync: this,
    duration: const Duration(milliseconds: 240),
  );
  late final Animation<double> _fade = CurvedAnimation(
    parent: _c,
    curve: const Cubic(0.2, 0, 0, 1),
  );

  @override
  void initState() {
    super.initState();
    final delay = Duration(milliseconds: 24 * math.min(widget.index, 8));
    Future<void>.delayed(delay, () {
      if (mounted) _c.forward();
    });
  }

  @override
  void dispose() {
    _c.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => AnimatedBuilder(
    animation: _fade,
    builder: (context, child) => Opacity(
      opacity: _fade.value,
      child: Transform.translate(
        offset: Offset(0, (1 - _fade.value) * 8),
        child: child,
      ),
    ),
    child: widget.child,
  );
}

/// Page transition: the incoming screen is revealed through the same ordered
/// dither the covers use, so navigation and artwork share one language.
class PixelPageRoute<TResult> extends PageRouteBuilder<TResult> {
  PixelPageRoute({required WidgetBuilder builder, super.settings})
    : super(
        transitionDuration: const Duration(milliseconds: 360),
        reverseTransitionDuration: const Duration(milliseconds: 280),
        pageBuilder: (context, _, _) => builder(context),
        transitionsBuilder: (context, animation, _, child) {
          final curved = CurvedAnimation(
            parent: animation,
            curve: Curves.easeOutCubic,
            reverseCurve: Curves.easeInCubic,
          );
          return AnimatedBuilder(
            animation: curved,
            builder: (context, _) =>
                ClipPath(clipper: BayerClipper(curved.value), child: child),
          );
        },
      );
}

/// Rebuilds a subtree once per display frame while audio is playing, so
/// anything driven by the player clock moves at the refresh rate rather than
/// the backend's notification rate.
class FrameTicker extends StatefulWidget {
  const FrameTicker({super.key, required this.playing, required this.builder});
  final bool playing;
  final WidgetBuilder builder;

  @override
  State<FrameTicker> createState() => _FrameTickerState();
}

class _FrameTickerState extends State<FrameTicker>
    with SingleTickerProviderStateMixin {
  late final Ticker _ticker = createTicker((_) {
    if (mounted) setState(() {});
  });

  @override
  void initState() {
    super.initState();
    if (widget.playing) _ticker.start();
  }

  @override
  void didUpdateWidget(FrameTicker old) {
    super.didUpdateWidget(old);
    if (widget.playing && !_ticker.isActive) {
      _ticker.start();
    } else if (!widget.playing && _ticker.isActive) {
      _ticker.stop();
    }
  }

  @override
  void dispose() {
    _ticker.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => widget.builder(context);
}

/// Small filled pill with an icon and an optional word. Replaces the bracket
/// text buttons on screens that carry primary actions.
class PillButton extends StatefulWidget {
  const PillButton({
    super.key,
    required this.icon,
    required this.label,
    required this.onTap,
    this.accent = C.signal,
  });

  final IconData icon;
  final String label;
  final VoidCallback onTap;
  final Color accent;

  @override
  State<PillButton> createState() => _PillButtonState();
}

class _PillButtonState extends State<PillButton> {
  bool _down = false;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    return GestureDetector(
      onTapDown: (_) => setState(() => _down = true),
      onTapCancel: () => setState(() => _down = false),
      onTapUp: (_) => setState(() => _down = false),
      onTap: widget.onTap,
      child: AnimatedScale(
        scale: _down ? 0.96 : 1,
        duration: const Duration(milliseconds: 110),
        child: Container(
          height: 34,
          padding: const EdgeInsets.symmetric(horizontal: S.md),
          decoration: BoxDecoration(
            color: widget.accent.withValues(alpha: 0.12),
            borderRadius: BorderRadius.circular(17),
          ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(widget.icon, size: 17, color: widget.accent),
              const SizedBox(width: 6),
              Text(widget.label, style: T.label(p.ink)),
            ],
          ),
        ),
      ),
    );
  }
}
