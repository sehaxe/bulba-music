import 'package:flutter/material.dart';

import '../api/client.dart';
import '../design/components.dart';
import '../design/tokens.dart';
import '../player/controller.dart';
import '../state/prefs.dart';

/// One page with everything that can be changed, grouped by what it affects.
/// Values that are not wired yet say so instead of pretending to work.
class SettingsScreen extends StatefulWidget {
  const SettingsScreen({
    super.key,
    required this.api,
    required this.player,
    required this.prefs,
    required this.themeMode,
    required this.onRescan,
  });

  final Api api;
  final PlayerController player;
  final Prefs prefs;
  final ValueNotifier<ThemeMode> themeMode;
  final Future<void> Function() onRescan;

  @override
  State<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends State<SettingsScreen> {
  String? _token;
  late final TextEditingController _serverController = TextEditingController(
    text: widget.api.base,
  );
  Map<String, dynamic>? _info;
  bool _renaming = false;
  String _embedNote = '';
  Map<String, dynamic>? _renamePreview;

  @override
  void initState() {
    super.initState();
    _token = widget.prefs.token;
    _load();
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
      if (mounted) setState(() => _embedNote = 'ошибка: $e');
    }
  }

  Future<void> _showPhrase(String phrase) async {
    final words = phrase.split(' ');
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
      if (mounted) {
        setState(() => _embedNote = 'войти не вышло: $e');
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
    if (mounted) setState(() => _embedNote = 'сервер: $trimmed');
  }

  Future<void> _previewRename() async {
    setState(() => _renaming = true);
    try {
      final out = await widget.api.normalize(apply: false);
      setState(() => _renamePreview = out);
    } catch (e) {
      setState(() => _embedNote = 'ошибка: $e');
    }
    if (mounted) setState(() => _renaming = false);
  }

  Future<void> _applyRename() async {
    setState(() => _renaming = true);
    try {
      final out = await widget.api.normalize(apply: true);
      setState(() {
        _renamePreview = out;
        _embedNote = 'переименовано: ${out['applied']}';
      });
      await _load();
    } catch (e) {
      setState(() => _embedNote = 'ошибка: $e');
    }
    if (mounted) setState(() => _renaming = false);
  }

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    final stats = _info?['stats'] as Map<String, dynamic>? ?? {};
    final libraries =
        (_info?['libraries'] as List<dynamic>?)?.map((e) => '$e').toList() ??
        const <String>[];
    final tracks = stats['tracks'] ?? 0;
    final albums = stats['albums'] ?? 0;
    final lossless = stats['lossless'] ?? 0;
    final embedded = stats['embedded'] ?? 0;
    final bytes = (stats['bytes'] as num?)?.toDouble() ?? 0;

    return ListView(
      padding: const EdgeInsets.fromLTRB(S.xl, S.xxl, S.xl, S.xxl),
      children: [
        Text('настройки', style: T.displayM(p.ink)),
        const SizedBox(height: S.xxl),

        _Section(
          title: 'библиотека',
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
            if (libraries.isEmpty)
              Text(
                'папки не заданы — сервер запущен без --library',
                style: T.caption(p.ash),
              ),
            const SizedBox(height: S.md),
            Text(
              'Сервер сам читает папку библиотеки при старте и каждые пять минут. '
              'Клиент не сканирует диск никогда — с телефона он о папках и не знает.',
              style: T.caption(p.mute),
            ),
            if (_embedNote.isNotEmpty) ...[
              const SizedBox(height: S.sm),
              Text(_embedNote, style: T.micro(p.mute)),
            ],
            const SizedBox(height: S.md),
            _Row(label: 'треков', value: '$tracks'),
            _Row(label: 'альбомов', value: '$albums'),
            _Row(label: 'lossless', value: '$lossless из $tracks'),
            _Row(label: 'размер', value: _bytes(bytes)),
            _Row(label: 'с признаками', value: '$embedded из $tracks'),
          ],
        ),

        _Section(
          title: 'имена файлов',
          children: [
            Text(
              'Внутри папки альбома — «01 - Название.flac», вне её — '
              '«Артист - Альбом - 01 - Название.flac». Существующие файлы '
              'не перезаписываются: совпадение это конфликт, а не слияние.',
              style: T.caption(p.mute),
            ),
            const SizedBox(height: S.md),
            Row(
              children: [
                PillButton(
                  icon: Icons.visibility_outlined,
                  label: _renaming ? '…' : 'посмотреть план',
                  onTap: _renaming ? null : _previewRename,
                ),
                const SizedBox(width: S.md),
                PillButton(
                  icon: Icons.drive_file_rename_outline_rounded,
                  label: 'переименовать',
                  onTap: _renaming ? null : _applyRename,
                ),
              ],
            ),
            if (_renamePreview != null) ...[
              const SizedBox(height: S.md),
              _Row(
                label: 'к переименованию',
                value:
                    '${_renamePreview!['planned'] is List ? (_renamePreview!['planned'] as List).length : 0}',
              ),
              _Row(
                label: 'конфликтов',
                value:
                    '${_renamePreview!['conflicts'] is List ? (_renamePreview!['conflicts'] as List).length : 0}',
              ),
              for (final r
                  in ((_renamePreview!['planned'] as List?) ?? const []).take(
                    6,
                  ))
                Padding(
                  padding: const EdgeInsets.only(top: 2),
                  child: Text(
                    '${('${r['from']}').split('/').last}  →  ${('${r['to']}').split('/').last}',
                    style: T.micro(p.ash),
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
            ],
          ],
        ),

        _Section(
          title: 'звук',
          children: [
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
            const SizedBox(height: S.sm),
            _Row(label: 'поток', value: 'flac с сервера · opus по требованию'),
          ],
        ),

        _Section(
          title: 'интерфейс',
          children: [
            ValueListenableBuilder<ThemeMode>(
              valueListenable: widget.themeMode,
              builder: (context, mode, _) => Row(
                children: [
                  for (final entry in const [
                    (ThemeMode.dark, 'тёмная'),
                    (ThemeMode.light, 'светлая'),
                    (ThemeMode.system, 'системная'),
                  ])
                    Padding(
                      padding: const EdgeInsets.only(right: S.sm),
                      child: _Choice(
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
                    ),
                ],
              ),
            ),
            const SizedBox(height: S.md),
            Text(
              'обложка задаёт акцент: палитра вынимается из арта и '
              'переходит вместе с треком.',
              style: T.micro(p.ash),
            ),
          ],
        ),

        _Section(
          title: 'клавиатура',
          children: const [
            _KeyRow(keys: 'space', what: 'плей / пауза'),
            _KeyRow(keys: '← →', what: 'назад / вперёд 5 с'),
            _KeyRow(keys: 'shift + ← →', what: '30 с'),
            _KeyRow(keys: '↑ ↓', what: 'громкость'),
            _KeyRow(keys: 'n / p', what: 'следующий / предыдущий'),
            _KeyRow(keys: 'l', what: 'лайк текущего'),
            _KeyRow(keys: 'q', what: 'очередь'),
            _KeyRow(keys: 'f', what: 'полный плеер'),
            _KeyRow(keys: '/', what: 'поиск'),
          ],
        ),

        _Section(
          title: 'синхронизация',
          children: [
            Text(
              'Одна фраза на все устройства: зарегистрируйся на компьютере, '
              'введи те же двенадцать слов на телефоне — и это один и тот же '
              'слушатель с историей, лайками и подборками.',
              style: T.caption(p.mute),
            ),
            const SizedBox(height: S.md),
            _Row(
              label: 'устройство',
              value: _token == null ? 'локальное, без аккаунта' : 'привязано',
            ),
            const SizedBox(height: S.sm),
            Row(
              children: [
                PillButton(
                  icon: Icons.key_rounded,
                  label: 'создать фразу',
                  onTap: _register,
                ),
                const SizedBox(width: S.md),
                PillButton(
                  icon: Icons.login_rounded,
                  label: 'ввести фразу',
                  onTap: _login,
                ),
              ],
            ),
          ],
        ),

        _Section(
          title: 'о системе',
          children: [
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
            _Row(label: 'версия', value: '${_info?['version'] ?? '—'}'),
            _Row(
              label: 'модель вкуса',
              value: '${_info?['features_model'] ?? 'dsp-v1'}',
            ),
            _Row(label: 'ревизия', value: '${stats['rev'] ?? '—'}'),
          ],
        ),
      ],
    );
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
                activeTrackColor: AccentScope.of(context),
                inactiveTrackColor: p.hairlineStrong,
                thumbColor: AccentScope.of(context),
                overlayColor: AccentScope.of(context).withValues(alpha: 0.12),
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
