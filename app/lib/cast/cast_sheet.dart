import 'package:flutter/material.dart';

import '../api/client.dart';
import '../design/components.dart';
import '../design/tokens.dart';
import '../player/controller.dart';
import 'cast.dart';
import 'cast_store.dart';

/// The cast sheet: who is on this network, and once connected, the three buttons
/// that matter.
Future<void> showCastSheet(
  BuildContext context, {
  required Api api,
  required PlayerController player,
  required CastStore cast,
}) async {
  final p = Palette.of(context);
  await showModalBottomSheet<void>(
    context: context,
    backgroundColor: p.canvas,
    isScrollControlled: true,
    showDragHandle: false,
    shape: const RoundedRectangleBorder(
      borderRadius: BorderRadius.vertical(top: Radius.circular(20)),
    ),
    builder: (_) => _CastSheet(api: api, player: player, cast: cast),
  );
}

class _CastSheet extends StatefulWidget {
  const _CastSheet({
    required this.api,
    required this.player,
    required this.cast,
  });

  final Api api;
  final PlayerController player;
  final CastStore cast;

  @override
  State<_CastSheet> createState() => _CastSheetState();
}

class _CastSheetState extends State<_CastSheet> {
  List<CastDevice> _devices = const [];
  bool _scanning = true;

  @override
  void initState() {
    super.initState();
    _scan();
  }

  Future<void> _scan() async {
    setState(() => _scanning = true);
    final found = <CastDevice>[];
    await for (final d in discover()) {
      if (!mounted) return;
      setState(() => _devices = [..._devices, d]);
      found.add(d);
    }
    if (mounted) setState(() => _scanning = false);
  }

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    return SafeArea(
      child: Padding(
        padding: const EdgeInsets.fromLTRB(S.lg, S.md, S.lg, S.lg),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Center(
              child: Container(
                width: 36,
                height: 4,
                margin: const EdgeInsets.only(bottom: S.md),
                decoration: BoxDecoration(
                  color: p.hairlineStrong,
                  borderRadius: BorderRadius.circular(2),
                ),
              ),
            ),
            Row(
              children: [
                Text('куда играть', style: T.heading(p.ink)),
                const SizedBox(width: S.sm),
                Expanded(child: Hr(color: p.hairline)),
                IconButton(
                  onPressed: _scanning ? null : _scan,
                  icon: Icon(
                    Icons.refresh_rounded,
                    size: 20,
                    color: _scanning ? p.hairlineStrong : p.mute,
                  ),
                ),
              ],
            ),
            if (widget.cast.active) ...[
              const SizedBox(height: S.sm),
              _NowCasting(cast: widget.cast),
            ],
            const SizedBox(height: S.sm),
            if (_devices.isEmpty && !_scanning)
              Padding(
                padding: const EdgeInsets.symmetric(vertical: S.md),
                child: Text(
                  'никого не видно · телевизор должен быть в той же сети',
                  style: T.caption(p.mute),
                ),
              ),
            for (final d in _devices)
              ListTile(
                contentPadding: EdgeInsets.zero,
                leading: Icon(
                  d.kind == CastKind.chromecast
                      ? Icons.cast_rounded
                      : Icons.tv_rounded,
                  color: widget.cast.device?.host == d.host
                      ? AccentScope.of(context)
                      : p.mute,
                ),
                title: Text(d.name, style: T.body(p.ink)),
                subtitle: Text('${d.label} · ${d.host}', style: T.micro(p.ash)),
                onTap: () async {
                  await widget.cast.connect(d);
                  if (!mounted) return;
                  setState(() {});
                },
              ),
            if (_scanning)
              Padding(
                padding: const EdgeInsets.symmetric(vertical: S.md),
                child: Text('ищу в сети…', style: T.caption(p.mute)),
              ),
            if (widget.cast.error != null)
              Padding(
                padding: const EdgeInsets.only(top: S.sm),
                child: Text(
                  widget.cast.error!,
                  style: T.micro(AccentScope.of(context)),
                ),
              ),
          ],
        ),
      ),
    );
  }
}

class _NowCasting extends StatefulWidget {
  const _NowCasting({required this.cast});

  final CastStore cast;

  @override
  State<_NowCasting> createState() => _NowCastingState();
}

class _NowCastingState extends State<_NowCasting> {
  CastStore get cast => widget.cast;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    final accent = AccentScope.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            Icon(Icons.cast_connected_rounded, size: 18, color: accent),
            const SizedBox(width: S.sm),
            Text(cast.device?.name ?? 'играет', style: T.bodyStrong(p.ink)),
            const Spacer(),
            IconButton(
              tooltip: 'отключить',
              onPressed: () async {
                await cast.disconnect();
                if (context.mounted) setState(() {});
              },
              icon: Icon(Icons.close_rounded, size: 20, color: p.mute),
            ),
          ],
        ),
        Row(
          children: [
            IconButton(
              onPressed: cast.pause,
              icon: Icon(Icons.pause_rounded, size: 26, color: p.ink),
            ),
            IconButton(
              onPressed: cast.play,
              icon: Icon(Icons.play_arrow_rounded, size: 30, color: accent),
            ),
          ],
        ),
      ],
    );
  }
}
