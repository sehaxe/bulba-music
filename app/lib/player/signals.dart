import 'dart:async';

import '../api/client.dart';

/// Buffers play events and ships them in batches. The log is append-only and
/// loss-tolerant: if the server is unreachable the batch stays in memory and is
/// retried, because listening history cannot be reconstructed later.
class SignalLog {
  SignalLog(this.api);

  final Api api;
  final List<Map<String, dynamic>> _pending = [];
  Timer? _timer;
  bool _sending = false;
  final int sessionId = DateTime.now().millisecondsSinceEpoch ~/ 1000;

  static const started = 0;
  static const progressed = 1;
  static const skipped = 2;
  static const completed = 3;
  static const repeated = 4;

  // Sources, matching the server enum.
  static const srcSearch = 0;
  static const srcAlbum = 1;
  static const srcArtist = 2;
  static const srcPlaylist = 3;
  static const srcRadio = 4;
  static const srcQueue = 5;

  void add(
    Track track, {
    required int event,
    required Duration position,
    required Duration length,
    int source = srcQueue,
    int? prevTrackId,
  }) {
    final ratio = length.inMilliseconds <= 0
        ? 0.0
        : (position.inMilliseconds / length.inMilliseconds).clamp(0.0, 1.0);
    _pending.add({
      'track_id': track.id,
      'session_id': sessionId,
      'event': event,
      'position_ms': position.inMilliseconds,
      'position_ratio': ratio,
      'source': source,
      'prev_track_id': prevTrackId,
      'device': 3, // desktop
      'offline': false,
      'quality': track.codec == 'flac' ? 0 : 1,
      'client_id': 'flutter-desktop',
    });
    if (_pending.length >= 2000) _pending.removeAt(0);
    _timer ??= Timer(const Duration(seconds: 6), flush);
  }

  Future<void> flush() async {
    _timer?.cancel();
    _timer = null;
    if (_sending || _pending.isEmpty) return;
    _sending = true;
    final batch = List<Map<String, dynamic>>.from(_pending);
    try {
      await api.postEvents(batch);
      _pending.removeRange(0, batch.length);
    } catch (_) {
      // Keep it; retry on the next tick.
      _timer ??= Timer(const Duration(seconds: 20), flush);
    } finally {
      _sending = false;
      if (_pending.isNotEmpty && _timer == null) {
        _timer = Timer(const Duration(seconds: 20), flush);
      }
    }
  }
}
