import 'dart:io';

import 'package:flutter/material.dart';

import '../api/client.dart';
import '../design/components.dart';
import '../design/tokens.dart';
import '../player/downloads.dart';
import 'duplicates.dart';
import '../player/controller.dart';
import '../player/likes.dart';
import '../state/prefs.dart';
import 'stats.dart';

/// Settings is a list of doors, not one long scroll: each category opens on
/// its own page and back returns to the list.
enum _Door { library, sound, looks, keys, sync, stats, about }

class SettingsScreen extends StatefulWidget {
  const SettingsScreen({
    super.key,
    required this.api,
    required this.player,
    required this.prefs,
    required this.themeMode,
    required this.likes,
    required this.onRescan,
    required this.onOpenAlbum,
    required this.onPlayTrack,
  });

  final Api api;
  final PlayerController player;
  final Prefs prefs;
  final ValueNotifier<ThemeMode> themeMode;
  final LikesStore likes;
  final Future<void> Function() onRescan;
  final void Function(int albumId, String title, String artist) onOpenAlbum;
  final void Function(StatTrack) onPlayTrack;

  @override
  State<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends State<SettingsScreen> {
  _Door? _open;
  Map<String, dynamic>? _info;
  String? _token;
  String _note = '';
  late final TextEditingController _serverController = TextEditingController(
    text: widget.api.base,
  );

  @override
  void initState() {
    super.initState();
    _token = widget.prefs.token;
    _load();
    // `BULBA_DOOR=sound` opens a pane on launch, so a settings block can be
    // reviewed with the same screenshot harness as every other screen.
    final door = Platform.environment['BULBA_DOOR'];
    if (door != null) {
      for (final d in _Door.values) {
        if (d.name == door) _open = d;
      }
    }
  }

  Future<void> _load() async {
    try {
      final info = await widget.api.info();
      if (mounted) setState(() => _info = info);
    } catch (_) {}
  }

  Future<void> _saveServer(String value) async {
    final trimmed = value.trim().replaceAll(RegExp(r'/+$'), '');
    if (trimmed.isEmpty) return;
    widget.api.base = trimmed;
    await widget.prefs.setServer(trimmed);
    await _load();
    if (mounted) setState(() => _note = 'сервер: $trimmed');
  }

  Future<void> _register() async {
    try {
      final session = await widget.api.register('слушатель');
      final phrase = session['phrase'] as String? ?? '';
      _token = session['token'] as String?;
      widget.api.token = _token;
      await widget.prefs.setToken(_token);
      if (!mounted) return;
      await _showPhrase(phrase);
    } catch (e) {
      if (mounted) setState(() => _note = 'ошибка: $e');
    }
  }

  Future<void> _showPhrase(String phrase) async {
    final words = phrase.split(' ').where((w) => w.isNotEmpty).toList();
    if (!mounted) return;
    await showDialog<void>(
      context: context,
      barrierDismissible: false,
      builder: (context) {
        final p = Palette.of(context);
        return Dialog(
          backgroundColor: p.roomElevated,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(16),
          ),
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 460),
            child: Padding(
              padding: const EdgeInsets.all(S.xl),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text('твоя фраза', style: T.heading(p.ink)),
                  const SizedBox(height: S.sm),
                  Text(
                    'Двенадцать слов — это и логин, и пароль. Покажи их один раз '
                    'себе: введёшь их на телефоне, и он подхватит библиотеку и вкус.',
                    style: T.caption(p.mute),
                  ),
                  const SizedBox(height: S.lg),
                  Wrap(
                    spacing: S.sm,
                    runSpacing: S.sm,
                    children: [
                      for (var i = 0; i < words.length; i++)
                        Container(
                          padding: const EdgeInsets.symmetric(
                            horizontal: 10,
                            vertical: 6,
                          ),
                          decoration: BoxDecoration(
                            color: p.card,
                            border: Border.all(color: p.hairline),
                            borderRadius: BorderRadius.circular(8),
                          ),
                          child: Text(
                            '${i + 1}. ${words[i]}',
                            style: T.body(p.ink),
                          ),
                        ),
                    ],
                  ),
                  const SizedBox(height: S.lg),
                  Align(
                    alignment: Alignment.centerRight,
                    child: PillButton(
                      icon: Icons.check_rounded,
                      label: 'записал',
                      onTap: () => Navigator.of(context).pop(),
                    ),
                  ),
                ],
              ),
            ),
          ),
        );
      },
    );
  }

  Future<void> _login() async {
    final controller = TextEditingController();
    final phrase = await showDialog<String>(
      context: context,
      builder: (context) {
        final p = Palette.of(context);
        return AlertDialog(
          backgroundColor: p.roomElevated,
          title: Text('вход по фразе', style: T.heading(p.ink)),
          content: TextField(
            controller: controller,
            autofocus: true,
            maxLines: 3,
            cursorColor: AccentScope.of(context),
            style: T.body(p.ink),
            decoration: InputDecoration(
              hintText: 'двенадцать слов через пробел',
              hintStyle: T.body(p.ash),
            ),
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(context).pop(),
              child: Text('отмена', style: T.label(p.mute)),
            ),
            TextButton(
              onPressed: () => Navigator.of(context).pop(controller.text),
              child: Text('войти', style: T.label(AccentScope.of(context))),
            ),
          ],
        );
      },
    );
    if (phrase == null || phrase.trim().isEmpty) return;
    try {
      final session = await widget.api.login(phrase.trim());
      _token = session['token'] as String?;
      widget.api.token = _token;
      await widget.prefs.setToken(_token);
      if (!mounted) return;
      await _load();
    } catch (e) {
      if (mounted) setState(() => _note = 'войти не вышло: $e');
    }
  }

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    if (_open != null) {
      return ListView(
        padding: const EdgeInsets.fromLTRB(S.xl, S.xl, S.xl, S.xxl),
        children: [
          Row(
            children: [
              _BackChip(onTap: () => setState(() => _open = null)),
              const SizedBox(width: S.md),
              Text(_title(_open!), style: T.displayM(p.ink)),
            ],
          ),
          const SizedBox(height: S.xl),
          ..._content(_open!, p),
        ],
      );
    }

    return ListView(
      padding: const EdgeInsets.fromLTRB(S.xl, S.xxl, S.xl, S.xxl),
      children: [
        Text('настройки', style: T.displayM(p.ink)),
        const SizedBox(height: S.xl),
        for (final door in _Door.values)
          _DoorRow(
            door: door,
            hint: _hint(door),
            onTap: () => setState(() => _open = door),
          ),
      ],
    );
  }

  String _title(_Door d) => switch (d) {
    _Door.library => 'библиотека',
    _Door.sound => 'звук',
    _Door.looks => 'интерфейс',
    _Door.keys => 'клавиатура',
    _Door.sync => 'синхронизация',
    _Door.stats => 'статистика',
    _Door.about => 'о системе',
  };

  String _hint(_Door d) {
    final stats = _info?['stats'] as Map<String, dynamic>? ?? {};
    return switch (d) {
      _Door.library =>
        '${stats['tracks'] ?? 0} треков · ${stats['albums'] ?? 0} альбомов',
      _Door.sound => 'громкость, кроссфейд, поток',
      _Door.looks => 'тёмная или светлая, акцент из обложки',
      _Door.keys => 'горячие клавиши',
      _Door.sync =>
        _token == null ? 'локальное устройство' : 'привязано к фразе',
      _Door.stats => 'что и когда играло',
      _Door.about => '${_info?['version'] ?? '—'} · ${widget.api.base}',
    };
  }

  List<Widget> _content(_Door door, Palette p) {
    final stats = _info?['stats'] as Map<String, dynamic>? ?? {};
    final libraries =
        (_info?['libraries'] as List<dynamic>?)?.map((e) => '$e').toList() ??
        const <String>[];
    final tracks = stats['tracks'] ?? 0;
    final albums = stats['albums'] ?? 0;
    final lossless = stats['lossless'] ?? 0;
    final bytes = (stats['bytes'] as num?)?.toDouble() ?? 0;

    switch (door) {
      case _Door.library:
        return [
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
          const SizedBox(height: S.md),
          Text(
            'Сервер сам читает папку библиотеки при старте и каждые пять минут. '
            'Клиент не сканирует диск никогда — с телефона он о папках и не знает.',
            style: T.caption(p.mute),
          ),
          const SizedBox(height: S.lg),
          _Row(label: 'треков', value: '$tracks'),
          _Row(label: 'альбомов', value: '$albums'),
          _Row(label: 'lossless', value: '$lossless из $tracks'),
          _Row(label: 'размер', value: _bytes(bytes)),
          _Row(
            label: 'с признаками',
            value: '${stats['embedded'] ?? 0} из $tracks',
          ),
          if (_note.isNotEmpty) ...[
            const SizedBox(height: S.sm),
            Text(_note, style: T.micro(p.mute)),
          ],
          const SizedBox(height: S.lg),
          Wrap(
            spacing: S.sm,
            runSpacing: S.sm,
            children: [
              _Choice(
                label: duplicateCount == null
                    ? 'дубли · проверить'
                    : duplicateCount == 0
                    ? 'дублей нет'
                    : 'дубли · $duplicateCount',
                selected: false,
                onTap: _openDuplicates,
              ),
            ],
          ),
          const SizedBox(height: S.sm),
          Text(
            'Один артист, попавший в библиотеку под двумя именами, делит историю '
            'прослушиваний и ломает радио: «Nirvana» и «NIRVANA (US)» — это одна '
            'страница, а не две.',
            style: T.micro(p.ash),
          ),
        ];

      case _Door.sound:
        return [
          _SliderRow(
            label: 'громкость',
            value: widget.player.volume,
            max: 1,
            display: '${(widget.player.volume * 100).round()}%',
            onChanged: (v) {
              widget.player.setVolume(v);
              widget.prefs.setVolume(v);
              setState(() {});
            },
          ),
          _SliderRow(
            label: 'кроссфейд',
            value: widget.prefs.crossfade.toDouble(),
            max: 12,
            display: widget.prefs.crossfade == 0
                ? 'выкл'
                : '${widget.prefs.crossfade} с',
            onChanged: (v) {
              widget.prefs.setCrossfade(v.round());
              widget.player.crossfade = v.round();
              setState(() {});
            },
          ),
          const SizedBox(height: S.xs),
          Text(
            'Два плеера: следующий трек стартует заранее на втором, громкость '
            'идёт по равномощной кривой cos/sin — на середине не проваливается '
            'на 3 дБ, как у линейного кроссфейда.',
            style: T.micro(p.ash),
          ),
          const SizedBox(height: S.lg),
          Text('выравнивание громкости', style: T.caption(p.mute)),
          const SizedBox(height: S.sm),
          Wrap(
            spacing: S.sm,
            runSpacing: S.sm,
            children: [
              _Choice(
                label: 'ReplayGain · до −14 LUFS',
                selected: widget.player.replayGain,
                onTap: () {
                  widget.player.replayGain = !widget.player.replayGain;
                  widget.prefs.setReplayGain(widget.player.replayGain);
                  widget.player.setVolume(widget.player.volume);
                  setState(() {});
                },
              ),
            ],
          ),
          const SizedBox(height: S.sm),
          Text(
            'Громкость выравнивается по EBU R128, посчитанной при импорте: тихий '
            'мастер больше не играет вдвое тише громкого. Сейчас '
            '${_gainLabel(widget.player.gainDb)} к текущему треку.',
            style: T.micro(p.ash),
          ),
          const SizedBox(height: S.lg),
          Text('эквалайзер', style: T.caption(p.mute)),
          const SizedBox(height: S.sm),
          Wrap(
            spacing: S.sm,
            runSpacing: S.sm,
            children: [
              for (final entry in const [
                ('flat', 'flat'),
                ('bass', 'бас'),
                ('vocal', 'вокал'),
                ('night', 'ночь'),
              ])
                _Choice(
                  label: entry.$2,
                  selected: widget.player.eqPreset == entry.$1,
                  onTap: () {
                    widget.player.setEq(entry.$1);
                    widget.prefs.setEq(entry.$1);
                    setState(() {});
                  },
                ),
            ],
          ),
          const SizedBox(height: S.sm),
          Text(
            'Десять полос в mpv: усиления ниже ±6 дБ, ширина 1.4 октавы. '
            '«ночь» срезает и верх, и низ — чтобы не будить соседей.',
            style: T.micro(p.ash),
          ),
          const SizedBox(height: S.lg),
          Text('поток', style: T.caption(p.mute)),
          const SizedBox(height: S.sm),
          Wrap(
            spacing: S.sm,
            runSpacing: S.sm,
            children: [
              for (final entry in const [
                ('flac', 'flac · мастер'),
                ('opus160', 'opus 160'),
                ('opus128', 'opus 128'),
                ('opus96', 'opus 96'),
              ])
                _Choice(
                  label: entry.$2,
                  selected: widget.prefs.quality == entry.$1,
                  onTap: () {
                    widget.prefs.setQuality(entry.$1);
                    widget.player.quality = entry.$1;
                    setState(() {});
                  },
                ),
            ],
          ),
          const SizedBox(height: S.sm),
          Text(
            'Opus готовится сервером один раз и кэшируется: 128k — это ~3 МБ на '
            'трек вместо 28 МБ FLAC, на телефонной сети разница в десять раз. '
            'Мастер при этом не трогается.',
            style: T.micro(p.ash),
          ),
          _Row(
            label: 'кэш',
            value: _bytes(((_info?['derived_bytes'] as num?) ?? 0).toDouble()),
          ),
        ];

      case _Door.looks:
        return [
          ValueListenableBuilder<ThemeMode>(
            valueListenable: widget.themeMode,
            builder: (context, mode, _) => Wrap(
              spacing: S.sm,
              runSpacing: S.sm,
              children: [
                for (final entry in const [
                  (ThemeMode.dark, 'тёмная'),
                  (ThemeMode.light, 'светлая'),
                  (ThemeMode.system, 'системная'),
                ])
                  _Choice(
                    label: entry.$2,
                    selected: mode == entry.$1,
                    onTap: () {
                      widget.themeMode.value = entry.$1;
                      widget.prefs.setThemeMode(switch (entry.$1) {
                        ThemeMode.light => 'light',
                        ThemeMode.system => 'system',
                        _ => 'dark',
                      });
                    },
                  ),
              ],
            ),
          ),
          const SizedBox(height: S.lg),
          Text(
            'Обложка задаёт акцент: палитра вынимается из арта и переходит на '
            'весь интерфейс вместе с треком.',
            style: T.micro(p.ash),
          ),
        ];

      case _Door.keys:
        return const [
          _KeyRow(keys: 'space', what: 'плей / пауза'),
          _KeyRow(keys: '← →', what: 'назад / вперёд 5 с'),
          _KeyRow(keys: 'shift + ← →', what: '30 с'),
          _KeyRow(keys: '↑ ↓', what: 'громкость'),
          _KeyRow(keys: 'n / p', what: 'следующий / предыдущий'),
          _KeyRow(keys: 'l', what: 'лайк текущего'),
          _KeyRow(keys: 'q', what: 'очередь'),
          _KeyRow(keys: 'f', what: 'полный плеер'),
          _KeyRow(keys: '/', what: 'поиск'),
        ];

      case _Door.sync:
        final dl = DownloadsScope.maybeOf(context);
        return [
          if (dl != null) ...[
            Text('на этом устройстве', style: T.caption(p.mute)),
            const SizedBox(height: S.sm),
            _Row(label: 'скачано', value: '${dl.count} треков'),
            _Row(label: 'занимают', value: _bytes(dl.bytes.toDouble())),
            const SizedBox(height: S.sm),
            Wrap(
              spacing: S.sm,
              runSpacing: S.sm,
              children: [
                PillButton(
                  icon: Icons.delete_sweep_rounded,
                  label: 'очистить загрузки',
                  onTap: dl.count == 0
                      ? null
                      : () async {
                          await dl.clear();
                          if (mounted) setState(() {});
                        },
                ),
              ],
            ),
            const SizedBox(height: S.sm),
            Text(
              'Скачанный трек играет с диска: сервер для него больше не нужен. '
              'Формат берётся из «звук → поток»: flac — мастер, opus 128 — почти '
              'в десять раз меньше.',
              style: T.micro(p.ash),
            ),
            const SizedBox(height: S.lg),
            Hr(color: p.hairline),
            const SizedBox(height: S.lg),
          ],
          Text(
            'Одна фраза на все устройства: зарегистрируйся на компьютере, введи '
            'те же двенадцать слов на телефоне — и это один и тот же слушатель с '
            'историей, лайками и подборками.',
            style: T.caption(p.mute),
          ),
          const SizedBox(height: S.md),
          _Row(
            label: 'устройство',
            value: _token == null ? 'локальное, без аккаунта' : 'привязано',
          ),
          const SizedBox(height: S.sm),
          Wrap(
            spacing: S.sm,
            runSpacing: S.sm,
            children: [
              PillButton(
                icon: Icons.key_rounded,
                label: 'создать фразу',
                onTap: _register,
              ),
              PillButton(
                icon: Icons.login_rounded,
                label: 'ввести фразу',
                onTap: _login,
              ),
            ],
          ),
        ];

      case _Door.stats:
        return [
          SizedBox(
            height: 900,
            child: StatsScreen(
              api: widget.api,
              onOpenAlbum: widget.onOpenAlbum,
              onPlayTrack: widget.onPlayTrack,
            ),
          ),
        ];

      case _Door.about:
        return [
          Row(
            children: [
              SizedBox(
                width: 170,
                child: Text('сервер', style: T.caption(p.mute)),
              ),
              Expanded(
                child: TextField(
                  controller: _serverController,
                  style: T.body(p.body),
                  cursorColor: AccentScope.of(context),
                  decoration: InputDecoration(
                    isDense: true,
                    hintText: 'http://100.x.y.z:8422',
                    hintStyle: T.body(p.ash),
                  ),
                  onSubmitted: _saveServer,
                ),
              ),
              const SizedBox(width: S.sm),
              PillButton(
                icon: Icons.save_rounded,
                label: 'сохранить',
                onTap: () => _saveServer(_serverController.text),
              ),
            ],
          ),
          const SizedBox(height: S.md),
          _Row(label: 'версия', value: '${_info?['version'] ?? '—'}'),
          _Row(
            label: 'модель вкуса',
            value: '${_info?['features_model'] ?? 'dsp-v1'}',
          ),
          _Row(label: 'ревизия', value: '${stats['rev'] ?? '—'}'),
          if (_note.isNotEmpty) ...[
            const SizedBox(height: S.sm),
            Text(_note, style: T.micro(p.mute)),
          ],
        ];
    }
  }

  int? duplicateCount;

  Future<void> _openDuplicates() async {
    try {
      final groups = await widget.api.duplicates();
      if (!mounted) return;
      setState(() => duplicateCount = groups.length);
      await Navigator.of(context).push(
        MaterialPageRoute<void>(
          builder: (_) => DuplicatesScreen(api: widget.api, initial: groups),
        ),
      );
      final after = await widget.api.duplicates();
      if (mounted) setState(() => duplicateCount = after.length);
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context)
            .showSnackBar(SnackBar(content: Text('не вышло: $e')));
      }
    }
  }

  static String _gainLabel(double db) {
    if (db.abs() < 0.05) return '0 дБ';
    return '${db > 0 ? '+' : ''}${db.toStringAsFixed(1)} дБ';
  }

  static String _bytes(double b) {
    if (b <= 0) return '—';
    const units = ['Б', 'КБ', 'МБ', 'ГБ', 'ТБ'];
    var v = b;
    var i = 0;
    while (v >= 1024 && i < units.length - 1) {
      v /= 1024;
      i++;
    }
    return '${v.toStringAsFixed(v >= 100 ? 0 : 1)} ${units[i]}';
  }
}

class _DoorRow extends StatefulWidget {
  const _DoorRow({required this.door, required this.hint, required this.onTap});
  final _Door door;
  final String hint;
  final VoidCallback onTap;

  @override
  State<_DoorRow> createState() => _DoorRowState();
}

class _DoorRowState extends State<_DoorRow> {
  bool _hover = false;

  static IconData _icon(_Door d) => switch (d) {
    _Door.library => Icons.folder_open_rounded,
    _Door.sound => Icons.graphic_eq_rounded,
    _Door.looks => Icons.palette_outlined,
    _Door.keys => Icons.keyboard_rounded,
    _Door.sync => Icons.sync_rounded,
    _Door.stats => Icons.insights_rounded,
    _Door.about => Icons.info_outline_rounded,
  };

  static String _label(_Door d) => switch (d) {
    _Door.library => 'библиотека',
    _Door.sound => 'звук',
    _Door.looks => 'интерфейс',
    _Door.keys => 'клавиатура',
    _Door.sync => 'синхронизация',
    _Door.stats => 'статистика',
    _Door.about => 'о системе',
  };

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    final accent = AccentScope.of(context);
    return MouseRegion(
      cursor: SystemMouseCursors.click,
      onEnter: (_) => setState(() => _hover = true),
      onExit: (_) => setState(() => _hover = false),
      child: GestureDetector(
        onTap: widget.onTap,
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 160),
          margin: const EdgeInsets.only(bottom: S.sm),
          padding: const EdgeInsets.symmetric(horizontal: S.lg, vertical: S.md),
          decoration: BoxDecoration(
            color: _hover ? p.soft : Colors.transparent,
            border: Border.all(color: p.hairline),
            borderRadius: BorderRadius.circular(12),
          ),
          child: Row(
            children: [
              Icon(_icon(widget.door), size: 18, color: accent),
              const SizedBox(width: S.md),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(_label(widget.door), style: T.bodyStrong(p.ink)),
                    Text(widget.hint, style: T.micro(p.ash)),
                  ],
                ),
              ),
              Icon(Icons.chevron_right_rounded, size: 18, color: p.mute),
            ],
          ),
        ),
      ),
    );
  }
}

class _BackChip extends StatelessWidget {
  const _BackChip({required this.onTap});
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    return MouseRegion(
      cursor: SystemMouseCursors.click,
      child: GestureDetector(
        onTap: onTap,
        child: Container(
          padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 7),
          decoration: BoxDecoration(
            border: Border.all(color: p.hairline),
            borderRadius: BorderRadius.circular(9),
          ),
          child: Icon(Icons.arrow_back_rounded, size: 16, color: p.mute),
        ),
      ),
    );
  }
}

class _Row extends StatelessWidget {
  const _Row({required this.label, required this.value});
  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 3),
      child: Row(
        children: [
          SizedBox(width: 170, child: Text(label, style: T.caption(p.mute))),
          Expanded(
            child: Text(
              value,
              style: T.body(p.body),
              overflow: TextOverflow.ellipsis,
            ),
          ),
        ],
      ),
    );
  }
}

class _KeyRow extends StatelessWidget {
  const _KeyRow({required this.keys, required this.what});
  final String keys;
  final String what;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 3),
      child: Row(
        children: [
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
            decoration: BoxDecoration(
              color: p.soft,
              border: Border.all(color: p.hairline),
              borderRadius: BorderRadius.circular(6),
            ),
            child: Text(keys, style: T.micro(p.ink)),
          ),
          const SizedBox(width: S.md),
          Text(what, style: T.caption(p.mute)),
        ],
      ),
    );
  }
}

class _SliderRow extends StatelessWidget {
  const _SliderRow({
    required this.label,
    required this.value,
    required this.max,
    required this.display,
    required this.onChanged,
  });
  final String label;
  final double value;
  final double max;
  final String display;
  final ValueChanged<double> onChanged;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    final accent = AccentScope.of(context);
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 4),
      child: Row(
        children: [
          SizedBox(width: 170, child: Text(label, style: T.caption(p.mute))),
          SizedBox(
            width: 220,
            child: SliderTheme(
              data: SliderTheme.of(context).copyWith(
                trackHeight: 3,
                thumbShape: const RoundSliderThumbShape(enabledThumbRadius: 6),
                overlayShape: const RoundSliderOverlayShape(overlayRadius: 12),
                activeTrackColor: accent,
                inactiveTrackColor: p.hairlineStrong,
                thumbColor: accent,
                overlayColor: accent.withValues(alpha: 0.12),
              ),
              child: Slider(
                value: value.clamp(0, max),
                max: max,
                onChanged: onChanged,
              ),
            ),
          ),
          const SizedBox(width: S.sm),
          Text(display, style: T.caption(p.body)),
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
