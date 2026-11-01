import 'package:flutter/material.dart';

import '../api/client.dart';
import '../design/components.dart';
import '../design/tokens.dart';

/// First run, three screens, no account, no sign-up. The only thing the app
/// genuinely needs from the listener is a library and a look.
class Onboarding extends StatefulWidget {
  const Onboarding({
    super.key,
    required this.api,
    required this.onDone,
    required this.onTheme,
    required this.initialTheme,
  });

  final Api api;
  final VoidCallback onDone;
  final ValueChanged<ThemeMode> onTheme;
  final ThemeMode initialTheme;

  @override
  State<Onboarding> createState() => _OnboardingState();
}

class _OnboardingState extends State<Onboarding> {
  final PageController _pages = PageController();
  int _page = 0;
  int _tracks = 0;
  int _albums = 0;
  bool _scanning = false;
  late ThemeMode _theme = widget.initialTheme;

  @override
  void initState() {
    super.initState();
    _loadInfo();
  }

  Future<void> _loadInfo() async {
    try {
      final info = await widget.api.info();
      final stats = info['stats'] as Map<String, dynamic>? ?? {};
      if (!mounted) return;
      setState(() {
        _tracks = (stats['tracks'] ?? 0) as int;
        _albums = (stats['albums'] ?? 0) as int;
      });
    } catch (_) {
      // A missing server is not an error here: the next screen explains it.
    }
  }

  Future<void> _scan() async {
    setState(() => _scanning = true);
    try {
      await widget.api.scan();
    } catch (_) {}
    await _loadInfo();
    if (mounted) setState(() => _scanning = false);
  }

  @override
  void dispose() {
    _pages.dispose();
    super.dispose();
  }

  void _next() {
    if (_page == 2) {
      widget.onDone();
      return;
    }
    _pages.nextPage(
      duration: const Duration(milliseconds: 420),
      curve: const Cubic(0.2, 0, 0, 1),
    );
  }

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    return Scaffold(
      backgroundColor: p.canvas,
      body: SafeArea(
        child: Column(
          children: [
            Expanded(
              child: PageView(
                controller: _pages,
                onPageChanged: (i) => setState(() => _page = i),
                children: [
                  _Welcome(p: p),
                  _Library(
                    p: p,
                    tracks: _tracks,
                    albums: _albums,
                    scanning: _scanning,
                    onScan: _scan,
                  ),
                  _Look(
                    p: p,
                    theme: _theme,
                    onTheme: (m) {
                      setState(() => _theme = m);
                      widget.onTheme(m);
                    },
                  ),
                ],
              ),
            ),
            Padding(
              padding: const EdgeInsets.fromLTRB(S.xl, 0, S.xl, S.xl),
              child: Row(
                children: [
                  for (var i = 0; i < 3; i++) ...[
                    AnimatedContainer(
                      duration: const Duration(milliseconds: 240),
                      width: i == _page ? 22 : 6,
                      height: 6,
                      margin: const EdgeInsets.only(right: 6),
                      decoration: BoxDecoration(
                        color: i == _page
                            ? AccentScope.of(context)
                            : p.hairlineStrong,
                        borderRadius: BorderRadius.circular(3),
                      ),
                    ),
                  ],
                  const Spacer(),
                  if (_page > 0)
                    TextButton(
                      onPressed: () => _pages.previousPage(
                        duration: const Duration(milliseconds: 320),
                        curve: Curves.easeOutCubic,
                      ),
                      child: Text('назад', style: T.label(p.mute)),
                    ),
                  const SizedBox(width: S.sm),
                  PillButton(
                    label: _page == 2 ? 'поехали' : 'дальше',
                    icon: _page == 2
                        ? Icons.play_arrow_rounded
                        : Icons.arrow_forward_rounded,
                    onTap: _next,
                  ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _Welcome extends StatelessWidget {
  const _Welcome({required this.p});
  final Palette p;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(S.xxl, S.xxl, S.xxl, S.xl),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Container(
            width: 72,
            height: 72,
            decoration: BoxDecoration(
              gradient: const LinearGradient(
                begin: Alignment.topLeft,
                end: Alignment.bottomRight,
                colors: [Color(0xFFF26B3A), Color(0xFFB8402A)],
              ),
              borderRadius: BorderRadius.circular(20),
            ),
            child: const Icon(
              Icons.graphic_eq_rounded,
              color: Colors.white,
              size: 34,
            ),
          ),
          const SizedBox(height: S.xl),
          Text('bulba music', style: T.hero(p.ink)),
          const SizedBox(height: S.md),
          SizedBox(
            width: 520,
            child: Text(
              'Твоя музыка, твои правила. Библиотека лежит у тебя, '
              'рекомендации считаются из того, что ты реально слушаешь, '
              'и ничего не уходит наружу.',
              style: T.body(p.mute),
            ),
          ),
          const SizedBox(height: S.xl),
          _Bullet(
            p: p,
            icon: Icons.album_rounded,
            text: 'flac и opus, без пережатия',
          ),
          _Bullet(
            p: p,
            icon: Icons.auto_awesome_rounded,
            text: 'подборки по настроению, а не по жанру',
          ),
          _Bullet(
            p: p,
            icon: Icons.offline_pin_rounded,
            text: 'работает без интернета',
          ),
        ],
      ),
    );
  }
}

class _Bullet extends StatelessWidget {
  const _Bullet({required this.p, required this.icon, required this.text});
  final Palette p;
  final IconData icon;
  final String text;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: S.sm),
      child: Row(
        children: [
          Icon(icon, size: 18, color: AccentScope.of(context)),
          const SizedBox(width: S.sm),
          Text(text, style: T.body(p.ink)),
        ],
      ),
    );
  }
}

class _Library extends StatelessWidget {
  const _Library({
    required this.p,
    required this.tracks,
    required this.albums,
    required this.scanning,
    required this.onScan,
  });
  final Palette p;
  final int tracks;
  final int albums;
  final bool scanning;
  final VoidCallback onScan;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(S.xxl, S.xxl, S.xxl, S.xl),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Text('библиотека', style: T.displayM(p.ink)),
          const SizedBox(height: S.sm),
          Text(
            'Сервер читает папки, которые ему указали. Теги, обложки и '
            'длительность подтягиваются сами.',
            style: T.body(p.mute),
          ),
          const SizedBox(height: S.xl),
          Row(
            children: [
              _Number(p: p, value: '$tracks', label: 'треков'),
              const SizedBox(width: S.xxl),
              _Number(p: p, value: '$albums', label: 'альбомов'),
            ],
          ),
          const SizedBox(height: S.xl),
          Text(
            'Сервер уже прочитал папку библиотеки. Дальше он обновляет её сам — '
            'ничего сканировать с телефона не нужно.',
            style: T.micro(p.ash),
          ),
        ],
      ),
    );
  }
}

class _Number extends StatelessWidget {
  const _Number({required this.p, required this.value, required this.label});
  final Palette p;
  final String value;
  final String label;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(value, style: T.displayL(p.ink)),
        Text(label, style: T.caption(p.mute)),
      ],
    );
  }
}

class _Look extends StatelessWidget {
  const _Look({required this.p, required this.theme, required this.onTheme});
  final Palette p;
  final ThemeMode theme;
  final ValueChanged<ThemeMode> onTheme;

  @override
  Widget build(BuildContext context) {
    final options = [
      (ThemeMode.dark, 'тёмная', Icons.dark_mode_rounded),
      (ThemeMode.light, 'светлая', Icons.light_mode_rounded),
      (ThemeMode.system, 'как система', Icons.brightness_auto_rounded),
    ];
    return Padding(
      padding: const EdgeInsets.fromLTRB(S.xxl, S.xxl, S.xxl, S.xl),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Text('как выглядит', style: T.displayM(p.ink)),
          const SizedBox(height: S.sm),
          Text(
            'Можно поменять в любой момент в настройках.',
            style: T.body(p.mute),
          ),
          const SizedBox(height: S.xl),
          Wrap(
            spacing: S.md,
            runSpacing: S.md,
            children: [
              for (final (mode, label, icon) in options)
                GestureDetector(
                  onTap: () => onTheme(mode),
                  child: AnimatedContainer(
                    duration: const Duration(milliseconds: 180),
                    width: 168,
                    padding: const EdgeInsets.all(S.md),
                    decoration: BoxDecoration(
                      color: theme == mode ? p.card : Colors.transparent,
                      border: Border.all(
                        color: theme == mode
                            ? AccentScope.of(context)
                            : p.hairline,
                        width: theme == mode ? 1.4 : 1,
                      ),
                      borderRadius: BorderRadius.circular(14),
                    ),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Icon(
                          icon,
                          size: 22,
                          color: theme == mode
                              ? AccentScope.of(context)
                              : p.mute,
                        ),
                        const SizedBox(height: S.md),
                        Text(label, style: T.bodyStrong(p.ink)),
                      ],
                    ),
                  ),
                ),
            ],
          ),
        ],
      ),
    );
  }
}
