import 'dart:async';
import 'dart:ui';

import 'package:flutter/material.dart';

import '../design/tokens.dart';

class NavItemData {
  const NavItemData(this.icon, this.label);
  final IconData icon;
  final String label;
}

const kNavItems = <NavItemData>[
  NavItemData(Icons.home_rounded, 'home'),
  NavItemData(Icons.search_rounded, 'search'),
  NavItemData(Icons.grid_view_rounded, 'library'),
  NavItemData(Icons.favorite_rounded, 'liked'),
  NavItemData(Icons.tune_rounded, 'settings'),
];

/// A floating capsule instead of a tab bar: the active pill slides between
/// icons with a spring, and the bar itself sits above the content like a dock.
class FloatingNav extends StatelessWidget {
  const FloatingNav({
    super.key,
    required this.index,
    required this.onSelect,
    this.accent,
  });

  final int index;
  final ValueChanged<int> onSelect;

  /// Null means "take the cover's colour from the tree".
  final Color? accent;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    final accent = this.accent ?? AccentScope.of(context);
    return Padding(
      padding: const EdgeInsets.fromLTRB(S.lg, 0, S.lg, S.md),
      child: ClipRRect(
        borderRadius: BorderRadius.circular(22),
        child: BackdropFilter(
          filter: ImageFilter.blur(sigmaX: 18, sigmaY: 18),
          child: Container(
            height: 64,
            decoration: BoxDecoration(
              color: p.canvas.withValues(alpha: 0.86),
              borderRadius: BorderRadius.circular(22),
              border: Border.all(color: p.hairline),
            ),
            child: LayoutBuilder(
              builder: (context, box) {
                final cell = box.maxWidth / kNavItems.length;
                return Stack(
                  children: [
                    AnimatedPositioned(
                      duration: const Duration(milliseconds: 320),
                      curve: Curves.easeOutBack,
                      left: cell * index + (cell - 52) / 2,
                      top: 8,
                      child: Container(
                        width: 52,
                        height: 48,
                        decoration: BoxDecoration(
                          color: accent.withValues(alpha: 0.16),
                          borderRadius: BorderRadius.circular(16),
                        ),
                        child: Icon(Icons.circle, size: 5, color: accent),
                      ),
                    ),
                    Row(
                      children: [
                        for (var i = 0; i < kNavItems.length; i++)
                          Expanded(
                            child: _NavIcon(
                              item: kNavItems[i],
                              active: i == index,
                              color: i == index ? accent : p.mute,
                              onTap: () => onSelect(i),
                            ),
                          ),
                      ],
                    ),
                  ],
                );
              },
            ),
          ),
        ),
      ),
    );
  }
}

class _NavIcon extends StatefulWidget {
  const _NavIcon({
    required this.item,
    required this.active,
    required this.color,
    required this.onTap,
  });
  final NavItemData item;
  final bool active;
  final Color color;
  final VoidCallback onTap;

  @override
  State<_NavIcon> createState() => _NavIconState();
}

class _NavIconState extends State<_NavIcon> {
  bool _down = false;

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      behavior: HitTestBehavior.opaque,
      onTapDown: (_) => setState(() => _down = true),
      onTapCancel: () => setState(() => _down = false),
      onTapUp: (_) => setState(() => _down = false),
      onTap: widget.onTap,
      child: AnimatedScale(
        scale: _down ? 0.88 : 1,
        duration: const Duration(milliseconds: 110),
        curve: Curves.easeOut,
        child: AnimatedScale(
          scale: widget.active ? 1.08 : 1,
          duration: const Duration(milliseconds: 220),
          curve: Curves.easeOutBack,
          child: Icon(widget.item.icon, size: 23, color: widget.color),
        ),
      ),
    );
  }
}

/// Desktop rail: same icons, but labelled and with a moving accent rule.
class NavRail extends StatelessWidget {
  const NavRail({
    super.key,
    required this.index,
    required this.onSelect,
    this.accent,
  });

  final int index;
  final ValueChanged<int> onSelect;

  /// Null means "take the cover's colour from the tree".
  final Color? accent;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    return SizedBox(
      width: 220,
      child: Padding(
        padding: const EdgeInsets.fromLTRB(S.md, S.xl, S.md, S.lg),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: S.md),
              child: Row(
                children: [
                  Container(
                    width: 22,
                    height: 22,
                    decoration: BoxDecoration(
                      color: accent,
                      borderRadius: BorderRadius.circular(7),
                    ),
                    child: const Icon(
                      Icons.graphic_eq,
                      size: 15,
                      color: Colors.white,
                    ),
                  ),
                  const SizedBox(width: S.sm),
                  Text('bulba music', style: T.bodyStrong(p.ink)),
                ],
              ),
            ),
            const SizedBox(height: S.xxl),
            for (var i = 0; i < kNavItems.length; i++)
              _RailItem(
                item: kNavItems[i],
                active: i == index,
                accent: accent,
                onTap: () => onSelect(i),
              ),
            const Spacer(),
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: S.md),
              child: Text('v0.1 · local', style: T.micro(p.ash)),
            ),
          ],
        ),
      ),
    );
  }
}

class _RailItem extends StatelessWidget {
  const _RailItem({
    required this.item,
    required this.active,
    required this.accent,
    required this.onTap,
  });

  final NavItemData item;
  final bool active;

  /// Null means "take the cover's colour from the tree".
  final Color? accent;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    final accent = this.accent ?? AccentScope.of(context);
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 1),
      child: Material(
        color: Colors.transparent,
        child: InkWell(
          borderRadius: BorderRadius.circular(10),
          onTap: onTap,
          child: AnimatedContainer(
            duration: const Duration(milliseconds: 160),
            padding: const EdgeInsets.symmetric(horizontal: S.md, vertical: 9),
            decoration: BoxDecoration(
              color: active
                  ? accent.withValues(alpha: 0.10)
                  : Colors.transparent,
              borderRadius: BorderRadius.circular(10),
            ),
            child: Row(
              children: [
                Icon(item.icon, size: 19, color: active ? accent : p.mute),
                const SizedBox(width: S.md),
                Text(item.label, style: T.label(active ? p.ink : p.mute)),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

/// Kept for the wordmark's caret if it is ever wanted again; the shell no
/// longer blinks anything in the chrome.
class Blink extends StatefulWidget {
  const Blink({super.key, required this.value, this.color = C.signal});
  final String value;
  final Color color;

  @override
  State<Blink> createState() => _BlinkState();
}

class _BlinkState extends State<Blink> with SingleTickerProviderStateMixin {
  late final AnimationController _c = AnimationController(
    vsync: this,
    duration: const Duration(milliseconds: 1000),
  )..repeat();
  Timer? _stop;

  @override
  void initState() {
    super.initState();
    _stop = Timer(const Duration(seconds: 6), () {
      _c.stop();
      _c.value = 1;
    });
  }

  @override
  void dispose() {
    _stop?.cancel();
    _c.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => AnimatedBuilder(
    animation: _c,
    builder: (_, _) => Opacity(
      opacity: _c.value < 0.5 ? 0.35 : 1,
      child: Text(widget.value, style: T.heading(widget.color)),
    ),
  );
}
