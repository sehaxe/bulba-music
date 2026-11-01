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
    _player.stream.position.listen((p) {
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
          _player.seek(Duration(milliseconds: _pendingSeek!.target));
        }
      }
      _position = p;
      _anchor = p;
      _anchorAt = DateTime.now();
      notifyListeners();
    });
    _player.stream.duration.listen((d) {
      if (d > Duration.zero) {
        _duration = d;
        notifyListeners();
      }
    });
    _player.stream.playing.listen((v) {
      _playing = v;
      _anchor = _position;
      _anchorAt = DateTime.now();
      notifyListeners();
    });
    _player.stream.completed.listen((done) {
      if (!done) return;
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
  final Player _player = Player();
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
    await _player.open(Media(api.stream(track.id).toString()));
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

  void toggle() => _player.playOrPause();

  Future<void> setVolume(double v) async {
    _volume = v.clamp(0.0, 1.0);
    notifyListeners();
    await _player.setVolume(_volume * 100);
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
        _player.stop();
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
    _player.open(Media(api.stream(track.id).toString()));
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
    await _player.seek(Duration(milliseconds: target));
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
    await _player.seek(Duration(milliseconds: target));
  }

  @override
  void dispose() {
    _progress?.cancel();
    signals.flush();
    _player.dispose();
    super.dispose();
  }
}
