import 'package:flutter/material.dart';

import '../api/client.dart';
import '../design/badges.dart';
import '../design/components.dart';
import '../design/tokens.dart';
import '../player/controller.dart';
import '../player/likes.dart';

/// The wave: an endless personal stream.
///
/// It is not a playlist that was computed once. Every batch is chosen from the
/// listener's own signals — plays, completions, likes, skips — and each row
/// says why it is there. Skipping and liking change the next batch, which is
/// the whole point of the tab.
class WaveScreen extends StatefulWidget {
  const WaveScreen({
    super.key,
    required this.api,
    required this.player,
    required this.likes,
    required this.accent,
    this.seed,
    this.seedTitle,
  });

  final Api api;
  final PlayerController player;
  final LikesStore likes;
  final Color accent;

  /// Start from this track; null means "from the taste alone".
  final int? seed;
  final String? seedTitle;

  @override
  State<WaveScreen> createState() => _WaveScreenState();
}

class _WaveScreenState extends State<WaveScreen> {
  List<WaveTrack> _upcoming = const [];
  bool _loading = true;
  bool _exhausted = false;
  bool _tasteReady = true;
  double _novelty = 0.35;
  String? _error;

  @override
  void initState() {
    super.initState();
    widget.player.addListener(_onPlayer);
    _start();
  }

  @override
  void didUpdateWidget(WaveScreen old) {
    super.didUpdateWidget(old);
    if (old.seed != widget.seed) _start();
  }

  @override
  void dispose() {
    widget.player.removeListener(_onPlayer);
    super.dispose();
  }

  /// Refill before the queue runs dry: the wave should never stop between
  /// batches.
  void _onPlayer() {
    if (_loading || _exhausted) return;
    final left = widget.player.queue.length - widget.player.queueIndex;
    if (widget.player.queueIndex >= 0 && left <= 3) _more();
  }

  Future<void> _start() async {
    setState(() {
      _loading = true;
      _error = null;
      _exhausted = false;
      _upcoming = const [];
    });
    try {
      final batch = await widget.api.wave(
        seed: widget.seed,
        novelty: _novelty,
        limit: 12,
      );
      if (!mounted) return;
      setState(() {
        _upcoming = batch.tracks;
        _tasteReady = batch.tasteReady;
        _loading = false;
        _exhausted = batch.tracks.isEmpty;
      });
      if (batch.tracks.isNotEmpty) {
        final tracks = batch.tracks.map((w) => w.track).toList();
        await widget.player.play(tracks.first, queue: tracks);
      }
    } catch (e) {
      if (mounted) {
        setState(() {
          _error = '$e';
          _loading = false;
        });
      }
    }
  }

  Future<void> _more() async {
    if (_exhausted) return;
    setState(() => _loading = true);
    try {
      final exclude = <int>{
        ...widget.player.queue.map((t) => t.id),
        if (widget.player.current != null) widget.player.current!.id,
      };
      final batch = await widget.api.wave(
        seed: widget.seed,
        novelty: _novelty,
        limit: 8,
        exclude: exclude,
      );
      if (!mounted) return;
      setState(() {
        _loading = false;
        _exhausted = batch.tracks.isEmpty;
        _upcoming = [..._upcoming, ...batch.tracks];
      });
      for (final item in batch.tracks) {
        widget.player.addToQueue(item.track);
      }
    } catch (_) {
      if (mounted) setState(() => _loading = false);
    }
  }

  Future<void> _restart({double? novelty, int? seed}) async {
    if (novelty != null) _novelty = novelty;
    widget.player.clearQueue();
    await _start();
  }

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    final current = widget.player.current;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(S.xl, S.xxl, S.xl, S.md),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.end,
            children: [
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Row(
                      children: [
                        Text('волна', style: T.displayM(p.ink)),
                        const SizedBox(width: S.md),
                        Icon(
                          Icons.waves_rounded,
                          size: 18,
                          color: widget.accent,
                        ),
                      ],
                    ),
                    const SizedBox(height: S.xxs),
                    Text(
                      widget.seedTitle != null
                          ? 'от «${widget.seedTitle}» · дальше решает вкус'
                          : _tasteReady
                          ? 'играет то, что подходит тебе сейчас'
                          : 'первый запуск: пропускай и лайкай, волна учится',
                      style: T.caption(p.mute),
                    ),
                  ],
                ),
              ),
              if (current != null && widget.player.playing)
                PillButton(
                  icon: Icons.pause_rounded,
                  label: 'пауза',
                  onTap: widget.player.toggle,
                )
              else
                PillButton(
                  icon: Icons.play_arrow_rounded,
                  label: 'играть',
                  onTap: () => _restart(),
                ),
              const SizedBox(width: S.sm),
              Tooltip(
                message: 'пересобрать',
                child: IconButton(
                  onPressed: () => _restart(),
                  icon: Icon(Icons.refresh_rounded, size: 20, color: p.mute),
                ),
              ),
            ],
          ),
        ),
        Padding(
          padding: const EdgeInsets.fromLTRB(S.xl, 0, S.xl, S.md),
          child: Row(
            children: [
              Text('знакомое', style: T.micro(p.ash)),
              Expanded(
                child: SliderTheme(
                  data: SliderTheme.of(context).copyWith(
                    trackHeight: 2,
                    thumbShape: const RoundSliderThumbShape(
                      enabledThumbRadius: 5,
                    ),
                    overlayShape: SliderComponentShape.noOverlay,
                    activeTrackColor: widget.accent,
                    inactiveTrackColor: p.hairlineStrong,
                    thumbColor: widget.accent,
                  ),
                  child: Slider(
                    value: _novelty,
                    min: 0,
                    max: 1,
                    onChanged: (v) => setState(() => _novelty = v),
                    onChangeEnd: (v) => _restart(novelty: v),
                  ),
                ),
              ),
              Text('новое', style: T.micro(p.ash)),
            ],
          ),
        ),
        Hr(color: p.hairlineStrong),
        Expanded(
          child: _error != null
              ? Center(
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Text('волна не отвечает', style: T.heading(p.ink)),
                      const SizedBox(height: S.sm),
                      SizedBox(
                        width: 320,
                        child: Text(
                          _error!,
                          textAlign: TextAlign.center,
                          style: T.caption(p.mute),
                        ),
                      ),
                      const SizedBox(height: S.md),
                      BracketButton('retry', onTap: _restart),
                    ],
                  ),
                )
              : _loading && _upcoming.isEmpty
              ? Center(child: Text('волна…', style: T.label(p.mute)))
              : ListView.builder(
                  padding: const EdgeInsets.only(bottom: S.xxl),
                  itemCount: _upcoming.length + 1,
                  itemBuilder: (context, i) {
                    if (i == _upcoming.length) {
                      return Padding(
                        padding: const EdgeInsets.all(S.xl),
                        child: Center(
                          child: Text(
                            _exhausted
                                ? 'библиотека кончилась'
                                : _loading
                                ? 'подыскиваю ещё…'
                                : '',
                            style: T.micro(p.ash),
                          ),
                        ),
                      );
                    }
                    final item = _upcoming[i];
                    final t = item.track;
                    final playing = current?.id == t.id;
                    return _WaveRow(
                      index: i,
                      track: t,
                      reason: item.reason,
                      playing: playing,
                      coverUrl: widget.api.cover(t.albumId, t.hasCover),
                      liked: widget.likes.contains(t.id),
                      accent: widget.accent,
                      onPlay: () => widget.player.play(
                        t,
                        queue: [..._upcoming.sublist(i).map((e) => e.track)],
                      ),
                      onLike: () => widget.likes.toggle(t),
                      onSkip: widget.player.next,
                    );
                  },
                ),
        ),
      ],
    );
  }
}

class _WaveRow extends StatefulWidget {
  const _WaveRow({
    required this.index,
    required this.track,
    required this.reason,
    required this.playing,
    required this.coverUrl,
    required this.liked,
    required this.accent,
    required this.onPlay,
    required this.onLike,
    required this.onSkip,
  });

  final int index;
  final Track track;
  final String reason;
  final bool playing;
  final String? coverUrl;
  final bool liked;
  final Color accent;
  final VoidCallback onPlay;
  final VoidCallback onLike;
  final VoidCallback onSkip;

  @override
  State<_WaveRow> createState() => _WaveRowState();
}

class _WaveRowState extends State<_WaveRow> {
  bool _hover = false;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    final t = widget.track;
    final active = widget.playing;
    return MouseRegion(
      cursor: SystemMouseCursors.click,
      onEnter: (_) => setState(() => _hover = true),
      onExit: (_) => setState(() => _hover = false),
      child: GestureDetector(
        onTap: widget.onPlay,
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 140),
          margin: const EdgeInsets.symmetric(horizontal: S.sm, vertical: 1),
          padding: const EdgeInsets.symmetric(horizontal: S.md, vertical: S.sm),
          decoration: BoxDecoration(
            color: active
                ? widget.accent.withValues(alpha: 0.08)
                : _hover
                ? p.soft
                : Colors.transparent,
            borderRadius: BorderRadius.circular(12),
          ),
          child: Row(
            children: [
              Stack(
                alignment: Alignment.center,
                children: [
                  CoverArt(url: widget.coverUrl, size: 46, animate: false),
                  if (active)
                    Container(
                      width: 46,
                      height: 46,
                      decoration: BoxDecoration(
                        color: Colors.black.withValues(alpha: 0.35),
                        borderRadius: BorderRadius.circular(8),
                      ),
                      child: Icon(
                        Icons.graphic_eq,
                        size: 20,
                        color: widget.accent,
                      ),
                    ),
                ],
              ),
              const SizedBox(width: S.md),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Row(
                      children: [
                        Flexible(
                          child: Text(
                            t.title,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: T.bodyStrong(active ? widget.accent : p.ink),
                          ),
                        ),
                        const SizedBox(width: 6),
                        TagStrip(track: t, max: 2, dense: true, color: p.ash),
                      ],
                    ),
                    Text(
                      t.artist,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: T.micro(p.mute),
                    ),
                    const SizedBox(height: 2),
                    Row(
                      children: [
                        Icon(
                          Icons.auto_awesome_rounded,
                          size: 10,
                          color: widget.accent.withValues(alpha: 0.8),
                        ),
                        const SizedBox(width: 4),
                        Flexible(
                          child: Text(
                            widget.reason,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: T.micro(p.ash),
                          ),
                        ),
                      ],
                    ),
                  ],
                ),
              ),
              const SizedBox(width: S.sm),
              AnimatedOpacity(
                duration: const Duration(milliseconds: 120),
                opacity: _hover || active ? 1 : 0,
                child: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    IconButton(
                      tooltip: widget.liked ? 'убрать лайк' : 'нравится',
                      onPressed: widget.onLike,
                      icon: Icon(
                        widget.liked
                            ? Icons.favorite_rounded
                            : Icons.favorite_border_rounded,
                        size: 17,
                        color: widget.liked ? widget.accent : p.mute,
                      ),
                    ),
                    IconButton(
                      tooltip: 'пропустить',
                      onPressed: widget.onSkip,
                      icon: Icon(Icons.close_rounded, size: 17, color: p.mute),
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
