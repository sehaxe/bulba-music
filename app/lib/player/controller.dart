import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:media_kit/media_kit.dart' show Media, Player;

import '../api/client.dart';
import 'signals.dart';

class _PendingSeek {
  _PendingSeek({
    required this.target,
    required this.corrections,
    required this.nominal,
  });
  final int target;
  final int corrections;
  final int nominal;
}

/// Owns the audio deck and the queue. The transition engine will sit between
/// this object and the output device; for now it opens one stream at a time and
/// reports its clock.
class PlayerController extends ChangeNotifier {
  PlayerController(this.api) : signals = SignalLog(api) {
    _bind(_deckA);
    _bind(_deckB);
    _deckA.setVolume(100);
    _deckB.setVolume(0);
  }

  /// Two decks, one output. Crossfade is not a special case in the UI: it is
  /// this object opening the next track early on the idle deck and riding the
  /// equal-power law between them.
  final Player _deckA = Player();
  final Player _deckB = Player();
  late Player _active = _deckA;
  Player get _idle => identical(_active, _deckA) ? _deckB : _deckA;

  /// Seconds of overlap, 0 disables it.
  int crossfade = 0;

  /// 'flac' streams the master; opus96/128/160 ask the server for a smaller
  /// copy, which is what a phone on mobile data should use.
  String quality = 'flac';
  Timer? _fade;
  bool _fading = false;

  void _bind(Player player) {
    player.stream.position.listen((p) {
      if (!identical(player, _active)) return;
      // A seek can land somewhere other than asked (some files carry wrong
      // duration headers). Correct once, quietly, instead of leaving the
      // listener far from where they tapped.
      final pending = _pendingSeek;
      if (pending != null) {
        final delta = p.inMilliseconds - pending.target;
        _pendingSeek = null;
        if (delta.abs() > 1200 && pending.corrections < 1) {
          _pendingSeek = _PendingSeek(
            target: pending.target - delta,
            corrections: pending.corrections + 1,
            nominal: pending.target,
          );
          player.seek(Duration(milliseconds: _pendingSeek!.target));
        }
      }
      _position = p;
      _anchor = p;
      _anchorAt = DateTime.now();
      notifyListeners();
      _maybeFade();
    });
    player.stream.duration.listen((d) {
      if (!identical(player, _active)) return;
      if (d > Duration.zero) {
        _duration = d;
        notifyListeners();
      }
    });
    player.stream.playing.listen((v) {
      if (!identical(player, _active)) return;
      _playing = v;
      _anchor = _position;
      _anchorAt = DateTime.now();
      notifyListeners();
    });
    player.stream.completed.listen((done) {
      if (!done || !identical(player, _active) || _fading) return;
      final t = current;
      if (t != null) {
        signals.add(
          t,
          event: SignalLog.completed,
          position: _duration,
          length: _duration,
        );
      }
      next();
    });
  }

  final Api api;
  final SignalLog signals;
  Timer? _progress;

  Track? current;
  List<Track> _queue = const [];
  int _index = -1;
  Duration _position = Duration.zero;
  Duration _anchor = Duration.zero;
  DateTime _anchorAt = DateTime.now();
  Duration _duration = Duration.zero;
  bool _playing = false;
  double _volume = 1.0;
  _PendingSeek? _pendingSeek;

  /// Position extrapolated between stream ticks, so the progress bar moves at
  /// the display refresh rate instead of the audio backend's reporting rate.
  Duration get position {
    if (!_playing) return _position;
    final p = _anchor + DateTime.now().difference(_anchorAt);
    return p > _duration ? _duration : p;
  }

  Duration get duration => _duration;
  bool get playing => _playing;
  List<Track> get queue => _queue;
  double get volume => _volume;
  int get queueIndex => _index;

  double get progress {
    final total = _duration.inMilliseconds;
    if (total <= 0) return 0;
    return (_position.inMilliseconds / total).clamp(0.0, 1.0);
  }

  Future<void> play(
    Track track, {
    List<Track>? queue,
    int source = SignalLog.srcQueue,

    /// Resume point, used by «продолжить» so a track picked up on another
    /// device starts where it stopped instead of at zero.
    Duration? startAt,
  }) async {
    final previous = current;
    if (previous != null && previous.id == track.id) {
      // Pressing play on what is already loaded, or restarting a track from the
      // list, is a deliberate repeat: the strongest signal of all after a like.
      signals.add(
        track,
        event: SignalLog.repeated,
        position: position,
        length: _duration,
        source: source,
      );
    }
    if (previous != null && previous.id != track.id) {
      // Leaving a track early is a skip only before it has been meaningfully
      // heard: under 20% or under 20 seconds.
      final ratio = _duration.inMilliseconds <= 0
          ? 0.0
          : position.inMilliseconds / _duration.inMilliseconds;
      if (ratio < 0.2 || position.inSeconds < 20) {
        signals.add(
          previous,
          event: SignalLog.skipped,
          position: position,
          length: _duration,
          source: source,
          prevTrackId: track.id,
        );
      }
    }
    if (queue != null) {
      _queue = queue;
      _index = queue.indexWhere((t) => t.id == track.id);
    }
    current = track;
    _position = Duration.zero;
    _anchor = Duration.zero;
    _anchorAt = DateTime.now();
    _duration = track.duration;
    notifyListeners();
    signals.add(
      track,
      event: SignalLog.started,
      position: Duration.zero,
      length: track.duration,
      source: source,
      prevTrackId: previous?.id,
    );
    _progress?.cancel();
    _progress = Timer.periodic(const Duration(seconds: 15), (_) {
      final t = current;
      if (t != null && _playing) {
        signals.add(
          t,
          event: SignalLog.progressed,
          position: position,
          length: _duration,
        );
      }
    });
    await _active.open(
      Media(api.stream(track.id, quality: quality).toString()),
    );
    if (startAt != null && startAt > Duration.zero) {
      final total = _duration.inMilliseconds;
      if (total > 0) await seek(startAt.inMilliseconds / total);
    }
    _cursor?.cancel();
    _cursor = Timer.periodic(const Duration(seconds: 10), (_) => _pushCursor());
  }

  /// The server keeps one cursor per listener so the phone can pick up what the
  /// desktop was playing. Fire and forget: a lost cursor is not a failed play.
  Timer? _cursor;

  Future<void> _pushCursor() async {
    final t = current;
    if (t == null || !_playing) return;
    try {
      await api.putCursor(t.id, position.inMilliseconds);
    } catch (_) {}
  }

  Future<void> next() async {
    if (_queue.isEmpty) return;
    final i = (_index + 1) % _queue.length;
    await play(_queue[i], queue: _queue);
  }

  Future<void> previous() async {
    if (_queue.isEmpty) return;
    if (_position.inSeconds > 3) {
      await seek(0);
      return;
    }
    final i = (_index - 1 + _queue.length) % _queue.length;
    await play(_queue[i], queue: _queue);
  }

  void toggle() => _active.playOrPause();

  Future<void> setVolume(double v) async {
    _volume = v.clamp(0.0, 1.0);
    notifyListeners();
    if (_fading) return; // the fade owns both decks while it runs
    await _active.setVolume(_volume * 100);
  }

  Future<void> toggleMute() => setVolume(_volume == 0 ? 1 : 0);

  /// Adds a track right after the current one. Radio and manual "play next"
  /// both go through here so the queue never has two sources of truth.
  void playNext(Track track) {
    final at = _index < 0 ? _queue.length : _index + 1;
    final next = [..._queue]..insert(at, track);
    _queue = next;
    notifyListeners();
  }

  /// Drops everything queued but keeps the current track playing.
  void clearQueue() {
    final t = current;
    _queue = t == null ? const [] : [t];
    _index = t == null ? -1 : 0;
    notifyListeners();
  }

  void addToQueue(Track track) {
    _queue = [..._queue, track];
    notifyListeners();
  }

  void removeAt(int index) {
    if (index < 0 || index >= _queue.length) return;
    final removed = _queue[index];
    final next = [..._queue]..removeAt(index);
    _queue = next;
    if (removed.id == current?.id) {
      // Removing the playing track moves to whatever now sits in its place.
      if (next.isNotEmpty) {
        _index = index.clamp(0, next.length - 1);
        _playWithoutHistory(next[_index]);
      } else {
        _index = -1;
        current = null;
        _active.stop();
        notifyListeners();
      }
      return;
    }
    if (index < _index) _index -= 1;
    notifyListeners();
  }

  void move(int from, int to) {
    if (from == to || from < 0 || from >= _queue.length) return;
    final next = [..._queue];
    final track = next.removeAt(from);
    next.insert(to.clamp(0, next.length), track);
    final playing = current?.id;
    _queue = next;
    _index = playing == null ? -1 : next.indexWhere((t) => t.id == playing);
    notifyListeners();
  }

  Future<void> jumpTo(int index) async {
    if (index < 0 || index >= _queue.length) return;
    await play(_queue[index], queue: _queue);
  }

  void _playWithoutHistory(Track track) {
    current = track;
    _position = Duration.zero;
    _anchor = Duration.zero;
    _anchorAt = DateTime.now();
    _duration = track.duration;
    notifyListeners();
    _active.open(Media(api.stream(track.id, quality: quality).toString()));
  }

  /// Nudges the position by delta without leaving the track; used by the
  /// keyboard shortcuts.
  Future<void> skipBy(Duration delta) async {
    final total = _duration.inMilliseconds;
    if (total <= 0) return;
    final target = (position.inMilliseconds + delta.inMilliseconds).clamp(
      0,
      total,
    );
    await _active.seek(Duration(milliseconds: target));
  }

  Future<void> seek(double ratio) async {
    final total = _duration.inMilliseconds;
    if (total <= 0) return;
    final target = (total * ratio.clamp(0.0, 1.0)).round();
    // Move the cursor and the clock at once: waiting for the decoder's next
    // tick made the player feel like it ignored the click.
    _position = Duration(milliseconds: target);
    _anchor = _position;
    _anchorAt = DateTime.now();
    _pendingSeek = _PendingSeek(
      target: target,
      corrections: 0,
      nominal: target,
    );
    notifyListeners();
    await _active.seek(Duration(milliseconds: target));
  }

  /// Fires once per track, when the remaining time reaches the crossfade
  /// length. Called from every position tick, so it has to be cheap and
  /// idempotent.
  void _maybeFade() {
    if (crossfade <= 0 || _fading || !_playing) return;
    final total = _duration.inMilliseconds;
    if (total <= 0) return;
    final left = total - _position.inMilliseconds;
    if (left > crossfade * 1000) return;
    if (left <= 0) return;
    if (_fadeStartedFor == current?.id) return;
    _fadeStartedFor = current?.id;
    _beginFade();
  }

  int? _fadeStartedFor;

  /// Starts the next track on the idle deck and rides an equal-power curve
  /// between the two. Called from the position tick, once, when the remaining
  /// time of the playing track reaches the crossfade length.
  Future<void> _beginFade() async {
    if (_fading || _queue.isEmpty || crossfade <= 0) return;
    final nextIndex = (_index + 1) % _queue.length;
    if (nextIndex == _index) return;
    final incoming = _queue[nextIndex];
    _fading = true;

    debugPrint('[fade] start ${crossfade}s → ${incoming.title}');
    final outgoing = _active;
    final target = _idle;
    await target.setVolume(0);
    await target.open(
      Media(api.stream(incoming.id, quality: quality).toString()),
    );

    final total = Duration(seconds: crossfade);
    final started = DateTime.now();
    _fade?.cancel();
    _fade = Timer.periodic(const Duration(milliseconds: 50), (timer) async {
      final t =
          (DateTime.now().difference(started).inMilliseconds /
                  total.inMilliseconds)
              .clamp(0.0, 1.0);
      // cos/sin keep the summed power constant: a linear fade dips ~3 dB in
      // the middle, which is audible as a hole in the mix.
      final theta = t * 3.141592653589793 / 2;
      await outgoing.setVolume(_volume * 100 * _cos(theta));
      await target.setVolume(_volume * 100 * _sin(theta));
      if (t >= 1.0) {
        timer.cancel();
        await outgoing.pause();
        await outgoing.stop();
        _active = target;
        _idle.play();
        _index = nextIndex;
        current = incoming;
        _fadeStartedFor = null;
        _position = Duration.zero;
        _anchor = Duration.zero;
        _anchorAt = DateTime.now();
        _duration = incoming.duration;
        _playing = true;
        _fading = false;
        _pendingSeek = null;
        debugPrint('[fade] done → ${incoming.title}');
        notifyListeners();
      }
    });
  }

  static double _cos(double x) {
    // Dart has no dart:math import here on purpose: the widget layer is kept
    // free of extra imports, and this approximation is within 1e-14.
    var term = 1.0, sum = 1.0, n = 0;
    var xx = x * x;
    while (term.abs() > 1e-14 && n < 12) {
      term *= -xx / ((2 * n + 1) * (2 * n + 2));
      sum += term;
      n++;
    }
    return sum;
  }

  static double _sin(double x) => _cos(1.5707963267948966 - x);

  @override
  void dispose() {
    _cursor?.cancel();
    _fade?.cancel();
    _progress?.cancel();
    signals.flush();
    _deckA.dispose();
    _deckB.dispose();
    super.dispose();
  }
}
