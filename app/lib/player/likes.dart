import 'package:flutter/foundation.dart';

import '../api/client.dart';

/// Tracks the set of liked track ids so every heart in the app can agree
/// without refetching. The server owns the truth; this is its mirror.
class LikesStore extends ChangeNotifier {
  LikesStore(this.api);

  final Api api;
  final Set<int> _ids = {};
  List<Track> _tracks = const [];
  bool _loading = false;

  Set<int> get ids => _ids;
  List<Track> get tracks => _tracks;
  bool get loading => _loading;
  bool contains(int id) => _ids.contains(id);

  Future<void> load() async {
    try {
      final ids = await api.likeIds();
      _ids
        ..clear()
        ..addAll(ids);
      notifyListeners();
    } catch (_) {
      // Offline: keep whatever we had.
    }
  }

  Future<void> refreshTracks() async {
    _loading = true;
    // Screens kick this off from initState, which runs *during* a build. A
    // synchronous notify there hits the ancestor listening widget mid-build and
    // throws "setState called during build" — so the spinner appears one frame
    // later instead.
    await Future<void>.delayed(Duration.zero);
    notifyListeners();
    try {
      _tracks = await api.likes();
    } catch (_) {
      _tracks = const [];
    }
    _loading = false;
    notifyListeners();
  }

  Future<void> toggle(Track track) async {
    final was = _ids.contains(track.id);
    if (was) {
      _ids.remove(track.id);
    } else {
      _ids.add(track.id);
    }
    notifyListeners();
    try {
      if (was) {
        await api.unlike(track.id);
      } else {
        await api.like(track.id);
      }
      if (!was) {
        _tracks = [track, ..._tracks.where((t) => t.id != track.id)];
      } else {
        _tracks = _tracks
            .where((t) => t.id != track.id)
            .toList(growable: false);
      }
      notifyListeners();
    } catch (_) {
      // Roll back so the heart never lies.
      if (was) {
        _ids.add(track.id);
      } else {
        _ids.remove(track.id);
      }
      notifyListeners();
    }
  }
}
