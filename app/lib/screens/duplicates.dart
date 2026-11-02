import 'package:flutter/material.dart';

import '../api/client.dart';
import '../design/components.dart';
import '../design/tokens.dart';

/// One screen for the mess a downloaded library always has: the same artist
/// under three spellings, the same record twice. Every group shows what will
/// actually happen — who survives and how many tracks move — so the merge is a
/// decision, not a leap.
class DuplicatesScreen extends StatefulWidget {
  const DuplicatesScreen({super.key, required this.api, this.initial});

  final Api api;
  final List<DupGroup>? initial;

  @override
  State<DuplicatesScreen> createState() => _DuplicatesScreenState();
}

class _DuplicatesScreenState extends State<DuplicatesScreen> {
  List<DupGroup>? _groups;
  String? _error;

  @override
  void initState() {
    super.initState();
    _groups = widget.initial;
    if (_groups == null) _load();
  }

  Future<void> _load() async {
    try {
      final g = await widget.api.duplicates();
      if (mounted) setState(() => _groups = g);
    } catch (e) {
      if (mounted) setState(() => _error = '$e');
    }
  }

  Future<void> _merge(DupGroup g, int keep, int lose) async {
    try {
      await widget.api.mergeDuplicates(kind: g.kind, keep: keep, merge: lose);
      await _load();
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context)
            .showSnackBar(SnackBar(content: Text('не вышло: $e')));
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    final groups = _groups;
    return Scaffold(
      backgroundColor: p.canvas,
      body: SafeArea(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(S.xl, S.lg, S.xl, S.sm),
              child: Row(
                children: [
                  _BackChip(onTap: () => Navigator.of(context).maybePop()),
                  const SizedBox(width: S.md),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text('дубли', style: T.displayM(p.ink)),
                        Text(
                          groups == null
                              ? 'смотрю библиотеку…'
                              : groups.isEmpty
                              ? 'всё чисто'
                              : '${groups.length} групп',
                          style: T.caption(p.mute),
                        ),
                      ],
                    ),
                  ),
                ],
              ),
            ),
            Hr(color: p.hairlineStrong),
            Expanded(
              child: _error != null
                  ? Center(child: Text(_error!, style: T.caption(p.mute)))
                  : groups == null
                  ? Center(child: Text('загружаю…', style: T.label(p.mute)))
                  : groups.isEmpty
                  ? Center(
                      child: Column(
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          Icon(Icons.check_rounded, size: 34, color: p.ash),
                          const SizedBox(height: S.md),
                          Text('ни одного дубля', style: T.label(p.mute)),
                        ],
                      ),
                    )
                  : ListView.builder(
                      padding: const EdgeInsets.fromLTRB(
                        S.xl,
                        S.md,
                        S.xl,
                        S.xxl,
                      ),
                      itemCount: groups.length,
                      itemBuilder: (context, i) => _GroupCard(
                        group: groups[i],
                        onMerge: (keep, lose) => _merge(groups[i], keep, lose),
                      ),
                    ),
            ),
          ],
        ),
      ),
    );
  }
}

class _GroupCard extends StatefulWidget {
  const _GroupCard({required this.group, required this.onMerge});
  final DupGroup group;
  final void Function(int keep, int lose) onMerge;

  @override
  State<_GroupCard> createState() => _GroupCardState();
}

class _GroupCardState extends State<_GroupCard> {
  int? _keep;

  @override
  void initState() {
    super.initState();
    _keep = widget.group.keep;
  }

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    final accent = AccentScope.of(context);
    final g = widget.group;
    final keep = _keep ?? g.keep;
    return Container(
      margin: const EdgeInsets.only(bottom: S.md),
      padding: const EdgeInsets.all(S.md),
      decoration: BoxDecoration(
        border: Border.all(color: p.hairline),
        borderRadius: BorderRadius.circular(14),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            g.kind == 'artist' ? 'один артист' : 'один альбом',
            style: T.micro(accent),
          ),
          const SizedBox(height: S.xs),
          for (final m in g.members)
            GestureDetector(
              onTap: () => setState(() => _keep = m.id),
              child: MouseRegion(
                cursor: SystemMouseCursors.click,
                child: Row(
                  children: [
                    Icon(
                      m.id == keep
                          ? Icons.radio_button_checked_rounded
                          : Icons.radio_button_off_rounded,
                      size: 16,
                      color: m.id == keep ? accent : p.ash,
                    ),
                    const SizedBox(width: S.sm),
                    Expanded(
                      child: Text(
                        m.name,
                        style: T.body(m.id == keep ? p.ink : p.mute),
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                    Text('${m.tracks} треков', style: T.caption(p.ash)),
                  ],
                ),
              ),
            ),
          const SizedBox(height: S.sm),
          Align(
            alignment: Alignment.centerRight,
            child: PillButton(
              icon: Icons.merge_rounded,
              label: 'слить',
              onTap: () {
                for (final m in g.members) {
                  if (m.id != keep) {
                    widget.onMerge(keep, m.id);
                    break;
                  }
                }
              },
            ),
          ),
        ],
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
    return GestureDetector(
      onTap: onTap,
      child: Container(
        width: 36,
        height: 36,
        decoration: BoxDecoration(
          border: Border.all(color: p.hairline),
          shape: BoxShape.circle,
        ),
        child: Icon(Icons.arrow_back_rounded, size: 18, color: p.mute),
      ),
    );
  }
}
