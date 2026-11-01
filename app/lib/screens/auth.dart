import 'package:flutter/material.dart';

import '../api/client.dart';
import '../design/components.dart';
import '../design/tokens.dart';
import '../player/likes.dart';
import '../player/controller.dart';

/// First screen on a device that is not the server's own machine.
///
/// A phone has no business seeing the desktop's history: it either starts its
/// own listener (a fresh twelve-word phrase) or joins the existing one by
/// typing the phrase the desktop showed once.
class AuthScreen extends StatefulWidget {
  const AuthScreen({
    super.key,
    required this.api,
    required this.player,
    required this.likes,
    required this.onSignedIn,
  });

  final Api api;
  final PlayerController player;
  final LikesStore likes;
  final void Function(String token) onSignedIn;

  @override
  State<AuthScreen> createState() => _AuthScreenState();
}

class _AuthScreenState extends State<AuthScreen> {
  bool _busy = false;
  String? _error;
  String? _phrase;
  final TextEditingController _entry = TextEditingController();

  Future<void> _create() async {
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      final out = await widget.api.register('слушатель');
      final token = out['token'] as String?;
      final phrase = out['phrase'] as String? ?? '';
      if (token == null) throw ApiException('сервер не выдал ключ');
      widget.api.token = token;
      widget.onSignedIn(token);
      if (!mounted) return;
      setState(() => _phrase = phrase);
    } catch (e) {
      if (mounted) setState(() => _error = '$e');
    }
    if (mounted) setState(() => _busy = false);
  }

  Future<void> _join() async {
    final text = _entry.text.trim();
    if (text.isEmpty) return;
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      final out = await widget.api.login(text);
      final token = out['token'] as String?;
      if (token == null) throw ApiException('фраза не подошла');
      widget.api.token = token;
      widget.onSignedIn(token);
    } catch (e) {
      if (mounted) setState(() => _error = '$e');
    }
    if (mounted) setState(() => _busy = false);
  }

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    final accent = AccentScope.of(context);
    final words = (_phrase ?? '')
        .split(' ')
        .where((w) => w.isNotEmpty)
        .toList();

    return Scaffold(
      backgroundColor: p.canvas,
      body: SafeArea(
        child: Center(
          child: SingleChildScrollView(
            padding: const EdgeInsets.all(S.xl),
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 460),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                mainAxisSize: MainAxisSize.min,
                children: [
                  Row(
                    children: [
                      Container(
                        width: 26,
                        height: 26,
                        decoration: BoxDecoration(
                          color: accent,
                          borderRadius: BorderRadius.circular(8),
                        ),
                        child: const Icon(
                          Icons.graphic_eq,
                          size: 17,
                          color: Colors.white,
                        ),
                      ),
                      const SizedBox(width: S.md),
                      Text('bulba music', style: T.bodyStrong(p.ink)),
                    ],
                  ),
                  const SizedBox(height: S.xxl),
                  if (_phrase == null) ...[
                    Text('кто слушает?', style: T.displayM(p.ink)),
                    const SizedBox(height: S.sm),
                    Text(
                      'Этот телефон ещё не подключён. Создай нового слушателя — '
                      'или введи двенадцать слов, которые компьютер показал при '
                      'регистрации, чтобы слушать вместе с ним.',
                      style: T.body(p.mute),
                    ),
                    const SizedBox(height: S.xl),
                    Row(
                      children: [
                        PillButton(
                          icon: Icons.auto_awesome_rounded,
                          label: _busy ? '…' : 'создать фразу',
                          onTap: _busy ? null : _create,
                        ),
                        const SizedBox(width: S.md),
                        Text('или', style: T.caption(p.ash)),
                      ],
                    ),
                    const SizedBox(height: S.xl),
                    TextField(
                      controller: _entry,
                      cursorColor: accent,
                      maxLines: 3,
                      style: T.body(p.ink),
                      decoration: InputDecoration(
                        hintText: 'двенадцать слов через пробел',
                        hintStyle: T.body(p.ash),
                        border: OutlineInputBorder(
                          borderRadius: BorderRadius.circular(10),
                          borderSide: BorderSide(color: p.hairline),
                        ),
                        enabledBorder: OutlineInputBorder(
                          borderRadius: BorderRadius.circular(10),
                          borderSide: BorderSide(color: p.hairline),
                        ),
                        focusedBorder: OutlineInputBorder(
                          borderRadius: BorderRadius.circular(10),
                          borderSide: BorderSide(color: accent),
                        ),
                      ),
                    ),
                    const SizedBox(height: S.md),
                    PillButton(
                      icon: Icons.login_rounded,
                      label: _busy ? '…' : 'войти',
                      onTap: _busy ? null : _join,
                    ),
                  ] else ...[
                    Text('твоя фраза', style: T.displayM(p.ink)),
                    const SizedBox(height: S.sm),
                    Text(
                      'Двенадцать слов — это и логин, и пароль. Введи их на '
                      'другом устройстве, и там будет эта же библиотека, история '
                      'и лайки. Больше они нигде не показываются.',
                      style: T.body(p.mute),
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
                              color: p.soft,
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
                    const SizedBox(height: S.xl),
                    PillButton(
                      icon: Icons.check_rounded,
                      label: 'записал, дальше',
                      onTap: () => widget.onSignedIn(widget.api.token ?? ''),
                    ),
                  ],
                  if (_error != null) ...[
                    const SizedBox(height: S.lg),
                    Text(_error!, style: T.caption(C.danger)),
                  ],
                  const SizedBox(height: S.xxl),
                  Text('Сервер: ${widget.api.base}', style: T.micro(p.ash)),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}
