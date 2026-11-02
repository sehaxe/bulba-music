import 'package:flutter/foundation.dart';

import '../api/client.dart';
import '../player/controller.dart';
import 'cast.dart';

/// Holds the one active cast target, and keeps it in step with the player.
///
/// Casting is not a second player: the phone stays the remote. Local audio
/// pauses, the device plays, and every track change reloads the URL there. When
/// the device drops off the network the store gives up rather than pretending.
class CastStore extends ChangeNotifier {
  CastStore({required this.api, required this.player}) {
    player.addListener(_onPlayer);
  }

  final Api api;
  final PlayerController player;

  CastTarget? target;
  CastDevice? device;
  String? error;
  int? _lastTrackId;

  bool get active => target != null;

  Future<void> connect(CastDevice d, {Track? track, String? quality}) async {
    await disconnect();
    try {
      final CastTarget t = d.kind == CastKind.chromecast
          ? await ChromecastConnection.connect(d)
          : await DlnaConnection.connect(d);
      target = t;
      device = d;
      error = null;
      final playing = track ?? player.current;
      if (playing != null) {
        _lastTrackId = playing.id;
        await t.load(
          api.stream(playing.id, quality: quality ?? player.quality).toString(),
          title: playing.title,
          artist: playing.artist,
        );
        // The phone is a remote now, not a second speaker.
        player.toggle();
      }
      notifyListeners();
    } catch (e) {
      error = e.toString();
      target = null;
      device = null;
      notifyListeners();
    }
  }

  Future<void> play() async {
    await target?.play();
    notifyListeners();
  }

  Future<void> pause() async {
    await target?.pause();
    notifyListeners();
  }

  Future<void> disconnect() async {
    final t = target;
    target = null;
    device = null;
    _lastTrackId = null;
    await t?.stop().catchError((_) {});
    await t?.close().catchError((_) {});
    notifyListeners();
  }

  /// Follow the player: a new track while casting is a new URL on the device.
  void _onPlayer() {
    final t = target;
    final track = player.current;
    if (t == null || track == null || track.id == _lastTrackId) return;
    _lastTrackId = track.id;
    t
        .load(
          api.stream(track.id, quality: player.quality).toString(),
          title: track.title,
          artist: track.artist,
        )
        .catchError((Object e) {
          error = e.toString();
          notifyListeners();
        });
  }

  @override
  void dispose() {
    player.removeListener(_onPlayer);
    target?.close().catchError((_) {});
    super.dispose();
  }
}
