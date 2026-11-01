import 'dart:math' as math;

import 'package:flutter/material.dart';

import '../api/client.dart';
import '../design/components.dart';
import '../design/tokens.dart';

/// The year in numbers. Every figure here comes from the same event log the
/// recommender reads, so the page cannot drift from what the app actually did.
class StatsScreen extends StatefulWidget {
  const StatsScreen({
    super.key,
    required this.api,
    required this.onOpenAlbum,
    required this.onPlayTrack,
  });

  final Api api;
  final void Function(int albumId, String title, String artist) onOpenAlbum;
  final void Function(StatTrack) onPlayTrack;

  @override
  State<StatsScreen> createState() => _StatsScreenState();
}

class _StatsScreenState extends State<StatsScreen> {
  Stats? _stats;
  String _period = 'all';
  bool _loading = true;
  String? _error;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    setState(() {
      _loading = true;
      _error = null;
    });
    try {
      final s = await widget.api.stats(period: _period);
      if (!mounted) return;
      setState(() {
        _stats = s;
        _loading = false;
      });
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _error = e.toString();
        _loading = false;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    final s = _stats;

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
                    Text('статистика', style: T.displayM(p.ink)),
                    const SizedBox(height: S.xxs),
                    Text(
                      s == null ? '' : (s.isEmpty ? 'тишина' : s.label),
                      style: T.caption(p.mute),
                    ),
                  ],
                ),
              ),
              _PeriodPicker(
                value: _period,
                onChanged: (v) {
                  setState(() => _period = v);
                  _load();
                },
              ),
            ],
          ),
        ),
        Hr(color: p.hairlineStrong),
        Expanded(
          child: _loading && s == null
              ? Center(child: Text('считаю…', style: T.label(p.mute)))
              : _error != null
              ? Center(
                  child: Text(
                    'offline · $_error',
                    style: T.label(C.danger),
                    textAlign: TextAlign.center,
                  ),
                )
              : s == null || s.isEmpty
              ? Center(
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Icon(Icons.insights_rounded, size: 34, color: p.ash),
                      const SizedBox(height: S.md),
                      Text('пока нечего считать', style: T.label(p.mute)),
                      const SizedBox(height: S.xs),
                      Text(
                        'включи пару треков — цифры появятся сами',
                        style: T.caption(p.ash),
                      ),
                    ],
                  ),
                )
              : _body(p, s),
        ),
      ],
    );
  }

  Widget _body(Palette p, Stats s) {
    final maxPlays = s.months.fold<int>(1, (a, m) => math.max(a, m.plays));
    return ListView(
      padding: const EdgeInsets.fromLTRB(S.xl, S.lg, S.xl, S.xxl),
      children: [
        // Hero numbers.
        Wrap(
          spacing: S.xxl,
          runSpacing: S.lg,
          children: [
            _Big(p: p, value: _hours(s.totals.hours), label: 'часов'),
            _Big(p: p, value: '${s.totals.plays}', label: 'включений'),
            _Big(p: p, value: '${s.totals.artists}', label: 'артистов'),
            _Big(p: p, value: '${s.totals.tracks}', label: 'треков'),
            _Big(p: p, value: '${s.totals.likes}', label: 'лайков'),
            _Big(p: p, value: '${s.daysPlayed}', label: 'дней'),
          ],
        ),
        if (s.longestStreak > 1) ...[
          const SizedBox(height: S.md),
          Text(
            'серия подряд: ${s.longestStreak} дн.',
            style: T.caption(p.mute),
          ),
        ],
        const SizedBox(height: S.xxl),

        // Months.
        Text('по месяцам', style: T.heading(p.ink)),
        const SizedBox(height: S.md),
        SizedBox(
          height: 140,
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.end,
            children: [
              for (final m in s.months)
                Expanded(
                  child: Padding(
                    padding: const EdgeInsets.symmetric(horizontal: 3),
                    child: Column(
                      mainAxisAlignment: MainAxisAlignment.end,
                      children: [
                        Text(
                          m.plays == 0 ? '' : '${m.plays}',
                          style: T.micro(p.ash),
                        ),
                        const SizedBox(height: 4),
                        TweenAnimationBuilder<double>(
                          tween: Tween(
                            begin: 0,
                            end: m.plays / maxPlays.clamp(1, 1 << 30),
                          ),
                          duration: const Duration(milliseconds: 620),
                          curve: Curves.easeOutCubic,
                          builder: (context, v, _) => Container(
                            height: math.max(2, 104 * v),
                            decoration: BoxDecoration(
                              color: m.plays == 0 ? p.hairline : C.signal,
                              borderRadius: const BorderRadius.vertical(
                                top: Radius.circular(2),
                              ),
                            ),
                          ),
                        ),
                        const SizedBox(height: 6),
                        Text(_monthName(m.month), style: T.micro(p.mute)),
                      ],
                    ),
                  ),
                ),
            ],
          ),
        ),
        const SizedBox(height: S.xxl),

        if (s.repeatTrack != null && s.repeatTrack!.repeats > 0) ...[
          Text('чаще всего на репите', style: T.heading(p.ink)),
          const SizedBox(height: S.sm),
          _RepeatCard(
            p: p,
            track: s.repeatTrack!,
            coverUrl: widget.api.cover(
              s.repeatTrack!.albumId,
              s.repeatTrack!.hasCover,
            ),
          ),
          const SizedBox(height: S.xxl),
        ],

        Text('топ треков', style: T.heading(p.ink)),
        const SizedBox(height: S.sm),
        for (var i = 0; i < s.topTracks.length; i++)
          _TrackLine(
            p: p,
            index: i + 1,
            track: s.topTracks[i],
            coverUrl: widget.api.cover(
              s.topTracks[i].albumId,
              s.topTracks[i].hasCover,
            ),
            onTap: () => widget.onPlayTrack(s.topTracks[i]),
          ),
        const SizedBox(height: S.xxl),

        if (s.topArtists.isNotEmpty) ...[
          Text('топ артистов', style: T.heading(p.ink)),
          const SizedBox(height: S.sm),
          for (var i = 0; i < s.topArtists.length; i++)
            _RankLine(
              p: p,
              index: i + 1,
              title: s.topArtists[i].name,
              value: '${s.topArtists[i].plays}',
            ),
          const SizedBox(height: S.xxl),
        ],

        if (s.topAlbums.isNotEmpty) ...[
          Text('топ альбомов', style: T.heading(p.ink)),
          const SizedBox(height: S.sm),
          for (var i = 0; i < s.topAlbums.length; i++)
            GestureDetector(
              behavior: HitTestBehavior.opaque,
              onTap: () => widget.onOpenAlbum(
                s.topAlbums[i].id,
                s.topAlbums[i].title,
                s.topAlbums[i].artist,
              ),
              child: Padding(
                padding: const EdgeInsets.symmetric(vertical: 5),
                child: Row(
                  children: [
                    SizedBox(
                      width: 26,
                      child: Text(
                        '${i + 1}',
                        textAlign: TextAlign.right,
                        style: T.caption(p.ash),
                      ),
                    ),
                    const SizedBox(width: S.md),
                    CoverArt(
                      url: widget.api.cover(
                        s.topAlbums[i].id,
                        s.topAlbums[i].hasCover,
                      ),
                      size: 36,
                    ),
                    const SizedBox(width: S.md),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            s.topAlbums[i].title,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: T.bodyStrong(p.ink),
                          ),
                          Text(s.topAlbums[i].artist, style: T.micro(p.mute)),
                        ],
                      ),
                    ),
                    Text('${s.topAlbums[i].plays}', style: T.caption(p.mute)),
                  ],
                ),
              ),
            ),
        ],
      ],
    );
  }

  static String _hours(double h) {
    if (h >= 10) return h.round().toString();
    return h.toStringAsFixed(1);
  }

  static String _monthName(int m) {
    const names = [
      'янв',
      'фев',
      'мар',
      'апр',
      'май',
      'июн',
      'июл',
      'авг',
      'сен',
      'окт',
      'ноя',
      'дек',
    ];
    return (m >= 1 && m <= 12) ? names[m - 1] : '';
  }
}

class _PeriodPicker extends StatelessWidget {
  const _PeriodPicker({required this.value, required this.onChanged});
  final String value;
  final ValueChanged<String> onChanged;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    const options = [('all', 'всё'), ('year', 'год'), ('month', 'месяц')];
    return Container(
      padding: const EdgeInsets.all(3),
      decoration: BoxDecoration(
        color: p.soft,
        border: Border.all(color: p.hairline),
        borderRadius: BorderRadius.circular(10),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          for (final (key, label) in options)
            GestureDetector(
              onTap: () => onChanged(key),
              child: AnimatedContainer(
                duration: const Duration(milliseconds: 160),
                padding: const EdgeInsets.symmetric(
                  horizontal: 12,
                  vertical: 6,
                ),
                decoration: BoxDecoration(
                  color: value == key ? p.card : Colors.transparent,
                  borderRadius: BorderRadius.circular(8),
                ),
                child: Text(
                  label,
                  style: T.label(value == key ? p.ink : p.mute),
                ),
              ),
            ),
        ],
      ),
    );
  }
}

class _Big extends StatelessWidget {
  const _Big({required this.p, required this.value, required this.label});
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

class _RepeatCard extends StatelessWidget {
  const _RepeatCard({
    required this.p,
    required this.track,
    required this.coverUrl,
  });
  final Palette p;
  final StatTrack track;
  final String? coverUrl;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(S.md),
      decoration: BoxDecoration(
        color: p.card,
        border: Border.all(color: p.hairline),
        borderRadius: BorderRadius.circular(14),
      ),
      child: Row(
        children: [
          CoverArt(url: coverUrl, size: 54, radius: 10),
          const SizedBox(width: S.md),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  track.title,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: T.bodyStrong(p.ink),
                ),
                Text(track.artist, style: T.caption(p.mute)),
              ],
            ),
          ),
          Column(
            crossAxisAlignment: CrossAxisAlignment.end,
            children: [
              Text('${track.repeats}×', style: T.displayM(C.signal)),
              Text('повторов', style: T.micro(p.mute)),
            ],
          ),
        ],
      ),
    );
  }
}

class _TrackLine extends StatelessWidget {
  const _TrackLine({
    required this.p,
    required this.index,
    required this.track,
    required this.coverUrl,
    required this.onTap,
  });
  final Palette p;
  final int index;
  final StatTrack track;
  final String? coverUrl;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      behavior: HitTestBehavior.opaque,
      onTap: onTap,
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: 5),
        child: Row(
          children: [
            SizedBox(
              width: 26,
              child: Text(
                '$index',
                textAlign: TextAlign.right,
                style: T.caption(p.ash),
              ),
            ),
            const SizedBox(width: S.md),
            CoverArt(url: coverUrl, size: 36),
            const SizedBox(width: S.md),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    track.title,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: T.bodyStrong(p.ink),
                  ),
                  Text(track.artist, style: T.micro(p.mute)),
                ],
              ),
            ),
            Text('${track.plays}×', style: T.caption(p.mute)),
            const SizedBox(width: S.md),
            SizedBox(
              width: 52,
              child: Text(
                track.hours > 0 ? '${track.hours.toStringAsFixed(1)} ч' : '',
                textAlign: TextAlign.right,
                style: T.micro(p.ash),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _RankLine extends StatelessWidget {
  const _RankLine({
    required this.p,
    required this.index,
    required this.title,
    required this.value,
  });
  final Palette p;
  final int index;
  final String title;
  final String value;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 5),
      child: Row(
        children: [
          SizedBox(
            width: 26,
            child: Text(
              '$index',
              textAlign: TextAlign.right,
              style: T.caption(p.ash),
            ),
          ),
          const SizedBox(width: S.md),
          Expanded(child: Text(title, style: T.body(p.ink))),
          Text(value, style: T.caption(p.mute)),
        ],
      ),
    );
  }
}
