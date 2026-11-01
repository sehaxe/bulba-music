import 'package:flutter/foundation.dart';
import 'package:media_kit/media_kit.dart' show Media, Player;

import '../api/client.dart';

/// Owns the audio deck and the queue. The transition engine will sit between
/// this object and the output device; for now it opens one stream at a time and
/// reports its clock.
class PlayerController extends ChangeNotifier {
  PlayerController(this.api) {
    _player.stream.position.listen((p) {
      _position = p;
      _anchor = p;
      _anchorAt = DateTime.now();
      notifyListeners();
    });
    _player.stream.duration.listen((d) {
      if (d > Duration.zero) _duration = d;
      notifyListeners();
    });
    _player.stream.playing.listen((v) {
      _playing = v;
      _anchor = _position;
      _anchorAt = DateTime.now();
      notifyListeners();
    });
    _player.stream.completed.listen((done) {
      if (done) next();
    });
  }

  final Api api;
  final Player _player = Player();

  Track? current;
  List<Track> _queue = const [];
  int _index = -1;
  Duration _position = Duration.zero;
  Duration _anchor = Duration.zero;
  DateTime _anchorAt = DateTime.now();
  Duration _duration = Duration.zero;
  bool _playing = false;
  double _volume = 1.0;

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

  Future<void> play(Track track, {List<Track>? queue}) async {
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

  Future<void> seek(double ratio) async {
    final total = _duration.inMilliseconds;
    if (total <= 0) return;
    await _player.seek(
      Duration(milliseconds: (total * ratio.clamp(0.0, 1.0)).round()),
    );
  }

  @override
  void dispose() {
    _player.dispose();
    super.dispose();
  }
}
