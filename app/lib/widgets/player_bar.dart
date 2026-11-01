import 'package:flutter/material.dart';

import '../api/client.dart';
import '../design/components.dart';
import '../design/tokens.dart';
import '../player/controller.dart';

/// Compact player docked above the navigation. Transport sits in the centre,
/// the track on the left, output level on the right — the layout every music
/// player has converged on, and for good reason.
class PlayerBar extends StatelessWidget {
  const PlayerBar({
    super.key,
    required this.player,
    required this.api,
    required this.onOpen,
    this.accent = C.signal,
    this.height = 64,
    this.showVolume = false,
  });

  final PlayerController player;
  final Api api;
  final VoidCallback onOpen;
  final Color accent;
  final double height;
  final bool showVolume;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    final t = player.current;
    if (t == null) return const SizedBox.shrink();
    return GestureDetector(
      onTap: onOpen,
      child: Container(
        height: height,
        decoration: BoxDecoration(
          color: p.canvas,
          border: Border(top: BorderSide(color: p.hairline)),
        ),
        child: Column(
          children: [
            SizedBox(
              height: 2,
              child: FrameTicker(
                playing: player.playing,
                builder: (context) => Stack(
                  children: [
                    Positioned.fill(child: ColoredBox(color: p.hairline)),
                    FractionallySizedBox(
                      widthFactor: player.progress,
                      child: ColoredBox(
                        color: accent,
                        child: const SizedBox.expand(),
                      ),
                    ),
                  ],
                ),
              ),
            ),
            Expanded(
              child: Padding(
                padding: const EdgeInsets.symmetric(horizontal: S.lg),
                child: Row(
                  children: [
                    Expanded(
                      child: _Info(t: t, api: api, player: player, p: p),
                    ),
                    Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        _CircleButton(
                          icon: Icons.skip_previous_rounded,
                          onTap: player.previous,
                          size: 34,
                        ),
                        const SizedBox(width: S.xxs),
                        _CircleButton(
                          icon: player.playing
                              ? Icons.pause_rounded
                              : Icons.play_arrow_rounded,
                          onTap: player.toggle,
                          size: 44,
                          filled: true,
                          accent: accent,
                        ),
                        const SizedBox(width: S.xxs),
                        _CircleButton(
                          icon: Icons.skip_next_rounded,
                          onTap: player.next,
                          size: 34,
                        ),
                      ],
                    ),
                    Expanded(
                      child: Align(
                        alignment: Alignment.centerRight,
                        child: showVolume
                            ? _Volume(player: player, accent: accent)
                            : const SizedBox.shrink(),
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _Info extends StatelessWidget {
  const _Info({
    required this.t,
    required this.api,
    required this.player,
    required this.p,
  });

  final Track t;
  final Api api;
  final PlayerController player;
  final Palette p;

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Hero(
          tag: 'now-cover',
          child: CoverArt(
            url: api.cover(t.albumId, t.hasCover),
            size: 42,
            radius: 8,
            animate: false,
          ),
        ),
        const SizedBox(width: S.md),
        Expanded(
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                t.title,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: T.bodyStrong(p.ink),
              ),
              FrameTicker(
                playing: player.playing,
                builder: (context) => Text(
                  '${t.artist} · ${mmss(player.position)}',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: T.micro(p.mute),
                ),
              ),
            ],
          ),
        ),
      ],
    );
  }
}

/// Speaker icon plus a hairline slider. Clicking the icon mutes.
class _Volume extends StatefulWidget {
  const _Volume({required this.player, required this.accent});
  final PlayerController player;
  final Color accent;

  @override
  State<_Volume> createState() => _VolumeState();
}

class _VolumeState extends State<_Volume> {
  bool _hover = false;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    final v = widget.player.volume;
    return MouseRegion(
      onEnter: (_) => setState(() => _hover = true),
      onExit: (_) => setState(() => _hover = false),
      child: Row(
        children: [
          GestureDetector(
            onTap: widget.player.toggleMute,
            child: Icon(
              v == 0
                  ? Icons.volume_off_rounded
                  : v < 0.5
                  ? Icons.volume_down_rounded
                  : Icons.volume_up_rounded,
              size: 20,
              color: _hover ? p.ink : p.mute,
            ),
          ),
          const SizedBox(width: S.sm),
          SizedBox(
            width: 96,
            height: 24,
            child: SliderTheme(
              data: SliderThemeData(
                trackHeight: 3,
                activeTrackColor: widget.accent,
                inactiveTrackColor: p.hairline,
                thumbColor: p.ink,
                overlayShape: SliderComponentShape.noOverlay,
                thumbShape: const RoundSliderThumbShape(enabledThumbRadius: 5),
                trackShape: const RoundedRectSliderTrackShape(),
              ),
              child: Slider(value: v, onChanged: widget.player.setVolume),
            ),
          ),
        ],
      ),
    );
  }
}

class _CircleButton extends StatefulWidget {
  const _CircleButton({
    required this.icon,
    required this.onTap,
    this.size = 40,
    this.filled = false,
    this.accent = C.signal,
  });

  final IconData icon;
  final VoidCallback? onTap;
  final double size;
  final bool filled;
  final Color accent;

  @override
  State<_CircleButton> createState() => _CircleButtonState();
}

class _CircleButtonState extends State<_CircleButton> {
  bool _down = false;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    return GestureDetector(
      behavior: HitTestBehavior.opaque,
      onTapDown: widget.onTap == null
          ? null
          : (_) => setState(() => _down = true),
      onTapCancel: () => setState(() => _down = false),
      onTapUp: widget.onTap == null
          ? null
          : (_) => setState(() => _down = false),
      onTap: widget.onTap,
      child: AnimatedScale(
        scale: _down ? 0.9 : 1,
        duration: const Duration(milliseconds: 110),
        curve: Curves.easeOut,
        child: Container(
          width: widget.size,
          height: widget.size,
          decoration: BoxDecoration(
            color: widget.filled ? widget.accent : Colors.transparent,
            shape: BoxShape.circle,
          ),
          child: Icon(
            widget.icon,
            size: widget.size * 0.55,
            color: widget.filled ? Colors.white : p.mute,
          ),
        ),
      ),
    );
  }
}
