import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/widgets.dart';
import 'package:path_provider/path_provider.dart';

import '../api/client.dart';

/// Makes the download index reachable from any widget without threading it
/// through a dozen constructors: the track menu needs it, and so does the player
/// bar, and neither owns it.
class DownloadsScope extends InheritedNotifier<Downloads> {
  const DownloadsScope({
    super.key,
    required Downloads downloads,
    required super.child,
  }) : super(notifier: downloads);

  static Downloads? maybeOf(BuildContext context) =>
      context.dependOnInheritedWidgetOfExactType<DownloadsScope>()?.notifier;

  static Downloads of(BuildContext context) =>
      maybeOf(context) ?? (throw StateError('no DownloadsScope'));
}

/// A track that exists on this device, with what it cost in space.
class DownloadEntry {
  DownloadEntry({
    required this.trackId,
    required this.file,
    required this.bytes,
    required this.at,
    this.quality = 'flac',
  });

  final int trackId;
  final String file;
  final int bytes;
  final int at;
  final String quality;

  Map<String, dynamic> toJson() => {
    'track_id': trackId,
    'file': file,
    'bytes': bytes,
    'at': at,
    'quality': quality,
  };

  factory DownloadEntry.fromJson(Map<String, dynamic> j) => DownloadEntry(
    trackId: j['track_id'] as int,
    file: j['file'] as String,
    bytes: (j['bytes'] ?? 0) as int,
    at: (j['at'] ?? 0) as int,
    quality: (j['quality'] ?? 'flac') as String,
  );
}

/// Files on this device, and the queue that fills them.
///
/// Downloads are ordinary files next to the app: the player opens them with a
/// file:// URL and never touches the server, which is the whole point of a
/// library that has to work on a train. The index is one JSON file, rewritten
/// after every change — a database for a few thousand rows would be a second
/// thing to corrupt.
class Downloads extends ChangeNotifier {
  Downloads(this.api);

  final Api api;
  Directory? _dir;
  final Map<int, DownloadEntry> _entries = {};
  final Set<int> _active = {};
  final Map<int, double> _progress = {};
  final List<(Track, String)> _pending = [];
  int _running = 0;
  static const int _parallel = 2;

  /// What a new download asks the server for. Set from the stream preference:
  /// the master costs 28 MB a track, the Opus render under 3.
  String defaultQuality = 'opus128';

  /// Soft cap. Bigger than this and the phone's own storage complaints start;
  /// smaller and a two-hour train ride runs dry.
  int quotaBytes = 8 * 1024 * 1024 * 1024;

  Iterable<DownloadEntry> get entries => _entries.values;
  int get count => _entries.length;
  int get bytes => _entries.values.fold(0, (a, e) => a + e.bytes);
  bool has(int trackId) => _entries.containsKey(trackId);
  String? pathFor(int trackId) => _entries[trackId]?.file;
  bool isDownloading(int trackId) => _active.contains(trackId);
  double progressOf(int trackId) => _progress[trackId] ?? 0;

  Future<void> load() async {
    final dir = await _ensureDir();
    final index = File('${dir.path}/index.json');
    if (!index.existsSync()) return;
    try {
      final data =
          jsonDecode(await index.readAsString()) as Map<String, dynamic>;
      _entries.clear();
      for (final e in (data['entries'] as List<dynamic>)) {
        final entry = DownloadEntry.fromJson(e as Map<String, dynamic>);
        // A file removed by the OS or by hand must not be advertised.
        if (File(entry.file).existsSync()) _entries[entry.trackId] = entry;
      }
      notifyListeners();
    } catch (_) {
      // A corrupt index is not worth a crash: forget it and keep the files.
    }
  }

  Future<Directory> _ensureDir() async {
    if (_dir != null) return _dir!;
    final base = await getApplicationSupportDirectory();
    final dir = Directory('${base.path}/downloads');
    if (!dir.existsSync()) dir.createSync(recursive: true);
    _dir = dir;
    return dir;
  }

  Future<void> _saveIndex() async {
    final dir = await _ensureDir();
    final payload = {
      'entries': [for (final e in _entries.values) e.toJson()],
    };
    await File('${dir.path}/index.json').writeAsString(jsonEncode(payload));
  }

  /// Queues a track. `quality` is the server's render name: `flac` for the
  /// master, `opus128` for a tenth of the size.
  void enqueue(Track track, {String quality = 'flac'}) {
    if (_entries.containsKey(track.id) ||
        _active.contains(track.id) ||
        _pending.any((p) => p.$1.id == track.id)) {
      return;
    }
    _pending.add((track, quality));
    _pump();
  }

  void enqueueAll(Iterable<Track> tracks, {String quality = 'flac'}) {
    for (final t in tracks) {
      enqueue(t, quality: quality);
    }
  }

  void _pump() {
    while (_running < _parallel && _pending.isNotEmpty) {
      final (track, quality) = _pending.removeAt(0);
      _running++;
      _active.add(track.id);
      _progress[track.id] = 0;
      notifyListeners();
      _download(track, quality).whenComplete(() {
        _running--;
        _active.remove(track.id);
        _progress.remove(track.id);
        notifyListeners();
        _pump();
      });
    }
  }

  /// Covers and waveforms are part of "offline", not decoration: without them a
  /// downloaded album opens as grey boxes and a flat line.
  Future<void> _cacheExtras(Track track) async {
    final dir = await _ensureDir();
    try {
      final coverDir = Directory('${dir.path}/covers');
      if (!coverDir.existsSync()) coverDir.createSync(recursive: true);
      final coverFile = File('${coverDir.path}/${track.albumId}.webp');
      if (track.hasCover && !coverFile.existsSync()) {
        final url = api.cover(track.albumId, true, w: 512);
        if (url != null) {
          final res = await HttpClient()
              .getUrl(Uri.parse(url))
              .then((r) => r.close());
          if (res.statusCode == 200) {
            await coverFile.writeAsBytes(
              await res.fold<List<int>>(<int>[], (a, b) => a..addAll(b)),
            );
          }
        }
      }
      final peaksDir = Directory('${dir.path}/peaks');
      if (!peaksDir.existsSync()) peaksDir.createSync(recursive: true);
      final peaksFile = File('${peaksDir.path}/${track.id}.json');
      if (!peaksFile.existsSync()) {
        final peaks = await api.peaks(track.id);
        await peaksFile.writeAsString(jsonEncode(peaks));
      }
    } catch (e) {
      debugPrint('cached extras for ${track.id} failed: $e');
    }
  }

  String? localCover(int albumId) {
    final dir = _dir;
    if (dir == null) return null;
    final f = File('${dir.path}/covers/$albumId.webp');
    return f.existsSync() ? f.path : null;
  }

  List<int>? localPeaks(int trackId) {
    final dir = _dir;
    if (dir == null) return null;
    final f = File('${dir.path}/peaks/$trackId.json');
    if (!f.existsSync()) return null;
    try {
      return (jsonDecode(f.readAsStringSync()) as List<dynamic>).cast<int>();
    } catch (_) {
      return null;
    }
  }

  Future<void> _download(Track track, String quality) async {
    final dir = await _ensureDir();
    final target = File(
      '${dir.path}/${track.id}.${quality == 'flac' ? 'flac' : 'opus'}',
    );
    final url = api.stream(track.id, quality: quality);
    final client = HttpClient();
    try {
      // Resume: a train tunnel is the normal case, not the exception.
      var have = target.existsSync() ? target.lengthSync() : 0;
      final req = await client.getUrl(url);
      if (have > 0) req.headers.set(HttpHeaders.rangeHeader, 'bytes=$have-');
      final res = await req.close();
      if (res.statusCode != 200 && res.statusCode != 206) {
        throw HttpException('download → ${res.statusCode}');
      }
      final total = res.contentLength > 0 ? have + res.contentLength : 0;
      final sink = target.openWrite(
        mode: have > 0 ? FileMode.append : FileMode.write,
      );
      var written = have;
      await for (final chunk in res) {
        sink.add(chunk);
        written += chunk.length;
        if (total > 0) {
          _progress[track.id] = written / total;
          notifyListeners();
        }
      }
      await sink.flush();
      await sink.close();
      _entries[track.id] = DownloadEntry(
        trackId: track.id,
        file: target.path,
        bytes: written,
        at: DateTime.now().millisecondsSinceEpoch ~/ 1000,
        quality: quality,
      );
      await _saveIndex();
      await _cacheExtras(track);
    } catch (e) {
      debugPrint('download failed for ${track.id}: $e');
    } finally {
      client.close();
    }
  }

  Future<void> remove(int trackId) async {
    final entry = _entries.remove(trackId);
    if (entry != null) {
      final f = File(entry.file);
      if (f.existsSync()) f.deleteSync();
      await _saveIndex();
      notifyListeners();
    }
  }

  Future<void> clear() async {
    for (final e in _entries.values) {
      final f = File(e.file);
      if (f.existsSync()) f.deleteSync();
    }
    _entries.clear();
    await _saveIndex();
    notifyListeners();
  }
}
