import 'dart:convert';

import 'package:http/http.dart' as http;

class Track {
  const Track({
    required this.id,
    required this.title,
    required this.artist,
    required this.album,
    required this.albumId,
    required this.hasCover,
    this.year,
    this.disc,
    this.trackNo,
    this.durationMs,
    this.bytes,
    this.codec,
    this.bitrate,
    this.sampleRate,
    this.bits,
    this.rev = 0,
  });

  final int id;
  final String title;
  final String artist;
  final String album;
  final int albumId;
  final bool hasCover;
  final int? year;
  final int? disc;
  final int? trackNo;
  final int? durationMs;
  final int? bytes;
  final String? codec;
  final int? bitrate;
  final int? sampleRate;
  final int? bits;
  final int rev;

  factory Track.fromJson(Map<String, dynamic> j) => Track(
    id: j['id'] as int,
    title: (j['title'] ?? 'untitled') as String,
    artist: (j['artist'] ?? '') as String,
    album: (j['album'] ?? '') as String,
    albumId: (j['album_id'] ?? 0) as int,
    hasCover: (j['has_cover'] ?? false) as bool,
    year: j['year'] as int?,
    disc: j['disc'] as int?,
    trackNo: j['track_no'] as int?,
    durationMs: j['duration_ms'] as int?,
    bytes: j['bytes'] as int?,
    codec: j['codec'] as String?,
    bitrate: j['bitrate'] as int?,
    sampleRate: j['sample_rate'] as int?,
    bits: j['bits'] as int?,
    rev: (j['rev'] ?? 0) as int,
  );

  Duration get duration => Duration(milliseconds: durationMs ?? 0);

  /// `flac 16/44`, `320`, `opus` — exactly what the file is, never a guess.
  String get format {
    final c = codec ?? '---';
    if (c == 'flac' || c == 'alac' || c == 'wav') {
      final b = bits;
      final r = sampleRate;
      if (b != null && r != null) return '$c $b/${(r / 1000).round()}';
      return c;
    }
    final br = bitrate;
    return br != null && br > 0 ? '$br' : c;
  }
}

class Mix {
  const Mix({
    required this.id,
    required this.title,
    required this.note,
    required this.tracks,
  });
  final String id;
  final String title;
  final String note;
  final List<Track> tracks;

  factory Mix.fromJson(Map<String, dynamic> j) => Mix(
    id: j['id'] as String,
    title: j['title'] as String,
    note: (j['note'] ?? '') as String,
    tracks: (j['tracks'] as List<dynamic>)
        .map((e) => Track.fromJson(e as Map<String, dynamic>))
        .toList(growable: false),
  );
}

class ApiException implements Exception {
  ApiException(this.message);
  final String message;
  @override
  String toString() => message;
}

class Api {
  Api(this.base);
  String base;
  final http.Client _http = http.Client();

  Future<List<Track>> tracks({int limit = 2000, int offset = 0}) async {
    final r = await _http
        .get(Uri.parse('$base/api/tracks?limit=$limit&offset=$offset'))
        .timeout(const Duration(seconds: 10));
    if (r.statusCode != 200) {
      throw ApiException('GET /tracks → ${r.statusCode}');
    }
    final list = jsonDecode(r.body) as List<dynamic>;
    return list
        .map((e) => Track.fromJson(e as Map<String, dynamic>))
        .toList(growable: false);
  }

  Future<Map<String, dynamic>> scan() async {
    final r = await _http
        .post(Uri.parse('$base/api/scan'))
        .timeout(const Duration(minutes: 30));
    if (r.statusCode != 200) {
      throw ApiException('scan → ${r.statusCode}');
    }
    return jsonDecode(r.body) as Map<String, dynamic>;
  }

  /// Incremental: computes embeddings only for tracks that lack one.
  Future<Map<String, dynamic>> buildFeatures({int limit = 300}) async {
    final r = await _http
        .post(Uri.parse('$base/api/features/build?limit=$limit'))
        .timeout(const Duration(minutes: 30));
    if (r.statusCode != 200) {
      throw ApiException('features → ${r.statusCode}');
    }
    return jsonDecode(r.body) as Map<String, dynamic>;
  }

  Future<Map<String, dynamic>> health() async {
    final r = await _http
        .get(Uri.parse('$base/api/health'))
        .timeout(const Duration(seconds: 5));
    if (r.statusCode != 200) {
      throw ApiException('health → ${r.statusCode}');
    }
    return jsonDecode(r.body) as Map<String, dynamic>;
  }

  Uri stream(int id) => Uri.parse('$base/api/tracks/$id/stream');

  Future<void> postEvents(List<Map<String, dynamic>> events) async {
    if (events.isEmpty) return;
    final r = await _http
        .post(
          Uri.parse('$base/api/events'),
          headers: {'content-type': 'application/json'},
          body: jsonEncode(events),
        )
        .timeout(const Duration(seconds: 15));
    if (r.statusCode != 200) {
      throw ApiException('events → ${r.statusCode}');
    }
  }

  Future<List<Track>> search(String query, {int limit = 60}) async {
    final r = await _http
        .get(
          Uri.parse(
            '$base/api/search?q=${Uri.encodeQueryComponent(query)}&limit=$limit',
          ),
        )
        .timeout(const Duration(seconds: 15));
    if (r.statusCode != 200) {
      throw ApiException('search → ${r.statusCode}');
    }
    final list = jsonDecode(r.body) as List<dynamic>;
    return list
        .map((e) => Track.fromJson(e as Map<String, dynamic>))
        .toList(growable: false);
  }

  Future<List<Mix>> dailyMixes() async {
    final r = await _http
        .get(Uri.parse('$base/api/recs/daily'))
        .timeout(const Duration(seconds: 30));
    if (r.statusCode != 200) {
      throw ApiException('recs/daily → ${r.statusCode}');
    }
    final list = jsonDecode(r.body) as List<dynamic>;
    return list
        .map((e) => Mix.fromJson(e as Map<String, dynamic>))
        .toList(growable: false);
  }

  Future<List<Mix>> homeMixes() async {
    final r = await _http
        .get(Uri.parse('$base/api/recs/home'))
        .timeout(const Duration(seconds: 30));
    if (r.statusCode != 200) {
      throw ApiException('recs/home → ${r.statusCode}');
    }
    final list = jsonDecode(r.body) as List<dynamic>;
    return list
        .map((e) => Mix.fromJson(e as Map<String, dynamic>))
        .toList(growable: false);
  }

  Future<List<Track>> similar(int id, {int n = 30}) async {
    final r = await _http
        .get(Uri.parse('$base/api/recs/similar/$id?n=$n'))
        .timeout(const Duration(seconds: 20));
    if (r.statusCode != 200) {
      throw ApiException('similar → ${r.statusCode}');
    }
    final list = jsonDecode(r.body) as List<dynamic>;
    return list
        .map((e) => Track.fromJson(e as Map<String, dynamic>))
        .toList(growable: false);
  }

  final Map<int, List<int>> _peaks = {};

  Future<List<int>> peaks(int id) async {
    final hit = _peaks[id];
    if (hit != null) return hit;
    final r = await _http
        .get(Uri.parse('$base/api/tracks/$id/peaks'))
        .timeout(const Duration(seconds: 30));
    if (r.statusCode != 200) {
      throw ApiException('peaks → ${r.statusCode}');
    }
    final list = (jsonDecode(r.body) as List<dynamic>).cast<int>();
    _peaks[id] = list;
    return list;
  }

  String? cover(int? albumId, bool hasCover) =>
      hasCover && albumId != null ? '$base/api/albums/$albumId/cover' : null;
}

/// A view over tracks that share an album. The catalogue has albums as rows on
/// the server; the client only needs this grouping for the home and album grids.
class Album {
  Album({
    required this.id,
    required this.title,
    required this.artist,
    required this.year,
    required this.hasCover,
    required this.tracks,
  });

  final int id;
  final String title;
  final String artist;
  final int? year;
  final bool hasCover;
  final List<Track> tracks;

  int get durationMs => tracks.fold(0, (sum, t) => sum + (t.durationMs ?? 0));
}

List<Album> albumsOf(List<Track> tracks) {
  final map = <int, List<Track>>{};
  for (final t in tracks) {
    map.putIfAbsent(t.albumId, () => []).add(t);
  }
  final albums = <Album>[];
  for (final entry in map.entries) {
    final list = entry.value
      ..sort(
        (a, b) => (a.disc ?? 0) != (b.disc ?? 0)
            ? (a.disc ?? 0) - (b.disc ?? 0)
            : (a.trackNo ?? 0) - (b.trackNo ?? 0),
      );
    final first = list.first;
    albums.add(
      Album(
        id: entry.key,
        title: first.album,
        artist: first.artist,
        year: first.year,
        hasCover: first.hasCover,
        tracks: list,
      ),
    );
  }
  albums.sort(
    (a, b) => a.artist.toLowerCase().compareTo(b.artist.toLowerCase()),
  );
  return albums;
}
