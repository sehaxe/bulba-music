import 'dart:async';
import 'dart:io';

import 'package:audio_service/audio_service.dart';
import 'package:flutter/foundation.dart';

import '../api/client.dart';
import 'controller.dart';

/// Android keeps a process alive only while a foreground service is running.
/// Without one the system suspends the app the moment the screen goes away and
/// libmpv stops with it — which is exactly what "music stops when I leave the
/// app" was. This handler exists to own that service and to feed the system the
/// lock-screen controls.
class BulbaAudioHandler extends BaseAudioHandler with QueueHandler {
  BulbaAudioHandler(this.player, this.api) {
    player.addListener(_onPlayer);
    _onPlayer();
    _ticker = Timer.periodic(const Duration(seconds: 1), (_) => _publish());
  }

  final PlayerController player;
  final Api api;
  Timer? _ticker;
  int? _lastTrack;
  bool _lastPlaying = false;

  void _onPlayer() {
    final t = player.current;
    if (t != null && t.id != _lastTrack) {
      _lastTrack = t.id;
      mediaItem.add(
        MediaItem(
          id: '${t.id}',
          title: t.title,
          artist: t.artist,
          album: t.album,
          duration: t.durationMs == null
              ? null
              : Duration(milliseconds: t.durationMs!),
          artUri: switch (api.cover(t.albumId, t.hasCover)) {
            final String url => Uri.parse(url),
            _ => null,
          },
        ),
      );
    }
    if (player.playing != _lastPlaying) {
      _lastPlaying = player.playing;
      _publish();
    }
  }

  void _publish() {
    final t = player.current;
    if (t == null) {
      playbackState.add(
        playbackState.value.copyWith(
          processingState: AudioProcessingState.idle,
          playing: false,
        ),
      );
      return;
    }
    playbackState.add(
      PlaybackState(
        controls: [
          MediaControl.skipToPrevious,
          if (player.playing) MediaControl.pause else MediaControl.play,
          MediaControl.skipToNext,
        ],
        systemActions: const {MediaAction.seek},
        androidCompactActionIndices: const [0, 1, 2],
        processingState: AudioProcessingState.ready,
        playing: player.playing,
        updatePosition: player.position,
        bufferedPosition: player.position,
        speed: 1.0,
        queueIndex: player.queueIndex < 0 ? null : player.queueIndex,
      ),
    );
  }

  @override
  Future<void> play() async {
    if (!player.playing) player.toggle();
    _publish();
  }

  @override
  Future<void> pause() async {
    if (player.playing) player.toggle();
    _publish();
  }

  @override
  Future<void> stop() async {
    if (player.playing) player.toggle();
    _publish();
    await super.stop();
  }

  @override
  Future<void> skipToNext() async {
    await player.next();
    _publish();
  }

  @override
  Future<void> skipToPrevious() async {
    await player.previous();
    _publish();
  }

  @override
  Future<void> seek(Duration position) async {
    final total = player.duration.inMilliseconds;
    if (total <= 0) return;
    await player.seek(position.inMilliseconds / total);
    _publish();
  }

  /// Called from the app's dispose; `BaseAudioHandler` has no dispose hook.
  void shutdown() {
    _ticker?.cancel();
    player.removeListener(_onPlayer);
  }
}

/// Boots the foreground service on Android. On every other platform playback
/// already survives a lost window, so there is nothing to start.
Future<void> startBackgroundAudio(PlayerController player, Api api) async {
  if (kIsWeb || !Platform.isAndroid) return;
  await AudioService.init(
    builder: () => BulbaAudioHandler(player, api),
    config: const AudioServiceConfig(
      androidNotificationChannelId: 'music.bulba.playback',
      androidNotificationChannelName: 'bulba music',
      // Keep the service (and therefore the process, and therefore the audio)
      // alive when the user pauses from the lock screen.
      // The package asserts that an "ongoing" notification is only legal when
      // the service also stops in the foreground on pause — which is the point
      // we are trying to avoid, so the notification stays dismissible and the
      // service stays alive.
      androidStopForegroundOnPause: false,
      androidNotificationOngoing: false,
      artDownscaleWidth: 512,
      artDownscaleHeight: 512,
    ),
  );
}
