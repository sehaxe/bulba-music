import 'package:flutter/material.dart';

import '../api/client.dart';
import '../design/components.dart';
import '../design/tokens.dart';

class SettingsScreen extends StatefulWidget {
  const SettingsScreen({
    super.key,
    required this.api,
    required this.themeMode,
    required this.onRescan,
  });

  final Api api;
  final ValueNotifier<ThemeMode> themeMode;
  final Future<void> Function() onRescan;

  @override
  State<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends State<SettingsScreen> {
  Map<String, dynamic>? _health;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    try {
      final health = await widget.api.health();
      if (mounted) setState(() => _health = health);
    } catch (_) {}
  }

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    final libraries =
        (_health?['libraries'] as List<dynamic>?)?.map((e) => '$e').toList() ??
        const <String>[];

    return ListView(
      padding: const EdgeInsets.fromLTRB(S.xl, S.xxl, S.xl, S.xxl),
      children: [
        Text('settings', style: T.displayM(p.ink)),
        const SizedBox(height: S.xxl),
        _Section(
          title: 'appearance',
          children: [
            ValueListenableBuilder<ThemeMode>(
              valueListenable: widget.themeMode,
              builder: (context, mode, _) => Row(
                children: [
                  for (final entry in const [
                    (ThemeMode.system, 'system'),
                    (ThemeMode.light, 'light'),
                    (ThemeMode.dark, 'dark'),
                  ])
                    Padding(
                      padding: const EdgeInsets.only(right: S.sm),
                      child: _Choice(
                        label: entry.$2,
                        selected: mode == entry.$1,
                        onTap: () => widget.themeMode.value = entry.$1,
                      ),
                    ),
                ],
              ),
            ),
          ],
        ),
        _Section(
          title: 'library',
          children: [
            for (final path in libraries)
              Padding(
                padding: const EdgeInsets.only(bottom: S.xs),
                child: Row(
                  children: [
                    Icon(Icons.folder_outlined, size: 16, color: p.mute),
                    const SizedBox(width: S.sm),
                    Expanded(
                      child: Text(
                        path,
                        style: T.caption(p.body),
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                  ],
                ),
              ),
            const SizedBox(height: S.sm),
            Row(
              children: [
                PillButton(
                  icon: Icons.refresh_rounded,
                  label: 'rescan',
                  onTap: () async {
                    await widget.onRescan();
                    await _load();
                  },
                ),
                const SizedBox(width: S.md),
                Text(
                  '${_health?['tracks'] ?? 0} tracks · ${_health?['albums'] ?? 0} albums',
                  style: T.caption(p.ash),
                ),
              ],
            ),
          ],
        ),
        _Section(
          title: 'playback',
          children: [
            _Row(
              label: 'crossfade',
              value: 'coming with the audio engine',
              muted: true,
            ),
            _Row(label: 'gapless', value: 'on', muted: true),
            _Row(label: 'streaming', value: 'flac · opus 128 on demand'),
          ],
        ),
        _Section(
          title: 'recommendations',
          children: [
            _Row(label: 'model', value: 'dsp-v1 (content)'),
            _Row(label: 'signals', value: 'play events, append-only'),
            _Row(label: 'next', value: 'taste prototypes, daily mixes'),
          ],
        ),
        _Section(
          title: 'about',
          children: [
            _Row(label: 'server', value: widget.api.base),
            _Row(label: 'version', value: '${_health?['version'] ?? '—'}'),
          ],
        ),
      ],
    );
  }
}

class _Section extends StatelessWidget {
  const _Section({required this.title, required this.children});
  final String title;
  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    return Padding(
      padding: const EdgeInsets.only(bottom: S.xxl),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Text(title, style: T.heading(p.ink)),
              const SizedBox(width: S.md),
              Expanded(child: Hr(color: p.hairline)),
            ],
          ),
          const SizedBox(height: S.md),
          ...children,
        ],
      ),
    );
  }
}

class _Row extends StatelessWidget {
  const _Row({required this.label, required this.value, this.muted = false});
  final String label;
  final String value;
  final bool muted;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 3),
      child: Row(
        children: [
          SizedBox(width: 160, child: Text(label, style: T.caption(p.mute))),
          Expanded(
            child: Text(
              value,
              style: T.body(muted ? p.ash : p.body),
              overflow: TextOverflow.ellipsis,
            ),
          ),
        ],
      ),
    );
  }
}

class _Choice extends StatelessWidget {
  const _Choice({
    required this.label,
    required this.selected,
    required this.onTap,
  });
  final String label;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    return GestureDetector(
      onTap: onTap,
      child: AnimatedContainer(
        duration: const Duration(milliseconds: 150),
        padding: const EdgeInsets.symmetric(horizontal: S.md, vertical: 7),
        decoration: BoxDecoration(
          color: selected ? p.ink : p.soft,
          borderRadius: BorderRadius.circular(9),
          border: Border.all(color: p.hairline),
        ),
        child: Text(label, style: T.label(selected ? p.canvas : p.mute)),
      ),
    );
  }
}
