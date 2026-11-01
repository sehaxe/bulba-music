import 'package:flutter/material.dart';

import '../api/client.dart';
import 'tokens.dart';

/// Small caps labels derived from things we actually know: the title and album
/// a rip came with, and the master's own format. Nothing is guessed from audio
/// — a "live" badge means the file says so.
enum TagKind {
  live,
  remix,
  acoustic,
  demo,
  instrumental,
  remaster,
  edit,
  deluxe,
  ost,
}

extension TagKindLabel on TagKind {
  String get label => switch (this) {
    TagKind.live => 'live',
    TagKind.remix => 'remix',
    TagKind.acoustic => 'acoustic',
    TagKind.demo => 'demo',
    TagKind.instrumental => 'instrumental',
    TagKind.remaster => 'remaster',
    TagKind.edit => 'edit',
    TagKind.deluxe => 'deluxe',
    TagKind.ost => 'ost',
  };
}

const _markers = <TagKind, List<String>>{
  TagKind.live: ['live', 'concert', 'unplugged', 'in concert'],
  TagKind.remix: ['remix', 'rmx', 'rework', 'flip'],
  TagKind.acoustic: ['acoustic', 'unacoustic'],
  TagKind.demo: ['demo', 'rough mix', 'outtake'],
  TagKind.instrumental: ['instrumental', 'inst.', 'karaoke'],
  TagKind.remaster: ['remaster', 'remastered'],
  TagKind.edit: ['radio edit', 'edit', 'single version'],
  TagKind.deluxe: ['deluxe', 'anniversary', 'expanded', 'bonus'],
  TagKind.ost: ['ost', 'soundtrack', 'original score', 'game soundtrack'],
};

/// Which markers a track carries, from its title and its album title.
List<TagKind> tagsOf(Track t) {
  final haystack = '${t.title} ${t.album}'.toLowerCase();
  final out = <TagKind>[];
  for (final entry in _markers.entries) {
    if (entry.value.any(haystack.contains)) out.add(entry.key);
  }
  return out;
}

/// Format facts, separated from content markers because they answer a different
/// question: what the file is, not what the recording is.
String formatBadge(Track t) {
  final bits = t.bits;
  final rate = t.sampleRate;
  final lossless = const [
    'flac',
    'alac',
    'wav',
    'aiff',
    'ape',
  ].contains((t.codec ?? '').toLowerCase());
  if (lossless && bits != null && bits >= 24) {
    return rate != null && rate >= 88200
        ? '$bits/${(rate / 1000).round()} hires'
        : '$bits bit';
  }
  if (lossless) return 'flac';
  if (t.bitrate != null && t.bitrate! >= 320) return '320';
  if (t.bitrate != null) return '${t.bitrate}';
  return (t.codec ?? '').toUpperCase();
}

/// Up to [max] tags, the rest summarised as +n.
class TagStrip extends StatelessWidget {
  const TagStrip({
    super.key,
    required this.track,
    this.max = 3,
    this.color,
    this.dense = false,
  });

  final Track track;
  final int max;
  final Color? color;
  final bool dense;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    final tags = tagsOf(track);
    if (tags.isEmpty) return const SizedBox.shrink();
    final shown = tags.take(max).toList();
    final rest = tags.length - shown.length;
    final ink = color ?? p.ash;
    return Wrap(
      spacing: 4,
      children: [
        for (final tag in shown)
          _Chip(label: tag.label, color: ink, dense: dense),
        if (rest > 0) _Chip(label: '+$rest', color: ink, dense: dense),
      ],
    );
  }
}

class _Chip extends StatelessWidget {
  const _Chip({required this.label, required this.color, required this.dense});
  final String label;
  final Color color;
  final bool dense;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: EdgeInsets.symmetric(
        horizontal: dense ? 4 : 6,
        vertical: dense ? 1 : 2,
      ),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.10),
        border: Border.all(color: color.withValues(alpha: 0.28)),
        borderRadius: BorderRadius.circular(5),
      ),
      child: Text(
        label,
        style: T
            .micro(color)
            .copyWith(fontSize: dense ? 9.5 : 10, letterSpacing: 0.02),
      ),
    );
  }
}
