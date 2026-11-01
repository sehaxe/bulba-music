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
    hasCover: j['has_cover'] == true || j['has_cover'] == 1,
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

class SearchAlbum {
  const SearchAlbum({
    required this.id,
    required this.title,
    required this.artist,
    required this.tracks,
    this.year,
    this.hasCover = false,
  });
  final int id;
  final String title;
  final String artist;
  final int tracks;
  final int? year;
  final bool hasCover;

  factory SearchAlbum.fromJson(Map<String, dynamic> j) => SearchAlbum(
    id: j['id'] as int,
    title: (j['title'] ?? '') as String,
    artist: (j['artist'] ?? '') as String,
    tracks: (j['tracks'] ?? 0) as int,
    year: j['year'] as int?,
    hasCover: j['has_cover'] == true || j['has_cover'] == 1,
  );
}

class SearchArtist {
  const SearchArtist({
    required this.id,
    required this.name,
    required this.albums,
    required this.tracks,
  });
  final int id;
  final String name;
  final int albums;
  final int tracks;

  factory SearchArtist.fromJson(Map<String, dynamic> j) => SearchArtist(
    id: j['id'] as int,
    name: (j['name'] ?? '') as String,
    albums: (j['albums'] ?? 0) as int,
    tracks: (j['tracks'] ?? 0) as int,
  );
}

class SearchResults {
  const SearchResults({
    required this.tracks,
    required this.albums,
    required this.artists,
    required this.query,
  });
  final List<Track> tracks;
  final List<SearchAlbum> albums;
  final List<SearchArtist> artists;
  final String query;

  bool get isEmpty => tracks.isEmpty && albums.isEmpty && artists.isEmpty;

  factory SearchResults.fromJson(Map<String, dynamic> j) => SearchResults(
    tracks: (j['tracks'] as List<dynamic>? ?? [])
        .map((e) => Track.fromJson(e as Map<String, dynamic>))
        .toList(growable: false),
    albums: (j['albums'] as List<dynamic>? ?? [])
        .map((e) => SearchAlbum.fromJson(e as Map<String, dynamic>))
        .toList(growable: false),
    artists: (j['artists'] as List<dynamic>? ?? [])
        .map((e) => SearchArtist.fromJson(e as Map<String, dynamic>))
        .toList(growable: false),
    query: (j['query'] ?? '') as String,
  );
}

class StatsTotals {
  const StatsTotals({
    required this.plays,
    required this.hours,
    required this.tracks,
    required this.artists,
    required this.likes,
    required this.skips,
    required this.repeats,
  });
  final int plays;
  final double hours;
  final int tracks;
  final int artists;
  final int likes;
  final int skips;
  final int repeats;

  factory StatsTotals.fromJson(Map<String, dynamic> j) => StatsTotals(
    plays: (j['plays'] ?? 0) as int,
    hours: ((j['hours'] ?? 0) as num).toDouble(),
    tracks: (j['tracks'] ?? 0) as int,
    artists: (j['artists'] ?? 0) as int,
    likes: (j['likes'] ?? 0) as int,
    skips: (j['skips'] ?? 0) as int,
    repeats: (j['repeats'] ?? 0) as int,
  );
}

class StatTrack {
  const StatTrack({
    required this.id,
    required this.title,
    required this.artist,
    required this.albumId,
    required this.hasCover,
    required this.plays,
    required this.repeats,
    required this.hours,
    this.album = '',
  });
  final int id;
  final String title;
  final String artist;
  final String album;
  final int albumId;
  final bool hasCover;
  final int plays;
  final int repeats;
  final double hours;

  factory StatTrack.fromJson(Map<String, dynamic> j) => StatTrack(
    id: j['id'] as int,
    title: (j['title'] ?? '') as String,
    artist: (j['artist'] ?? '') as String,
    albumId: (j['album_id'] ?? 0) as int,
    album: (j['album'] ?? '') as String,
    hasCover: j['has_cover'] == true || j['has_cover'] == 1,
    plays: (j['plays'] ?? 0) as int,
    repeats: (j['repeats'] ?? 0) as int,
    hours: ((j['hours'] ?? 0) as num).toDouble(),
  );
}

class StatArtist {
  const StatArtist({
    required this.name,
    required this.plays,
    required this.tracks,
  });
  final String name;
  final int plays;
  final int tracks;

  factory StatArtist.fromJson(Map<String, dynamic> j) => StatArtist(
    name: (j['name'] ?? '') as String,
    plays: (j['plays'] ?? 0) as int,
    tracks: (j['tracks'] ?? 0) as int,
  );
}

class StatAlbum {
  const StatAlbum({
    required this.id,
    required this.title,
    required this.artist,
    required this.hasCover,
    required this.plays,
  });
  final int id;
  final String title;
  final String artist;
  final bool hasCover;
  final int plays;

  factory StatAlbum.fromJson(Map<String, dynamic> j) => StatAlbum(
    id: j['id'] as int,
    title: (j['title'] ?? '') as String,
    artist: (j['artist'] ?? '') as String,
    hasCover: j['has_cover'] == true || j['has_cover'] == 1,
    plays: (j['plays'] ?? 0) as int,
  );
}

class MonthBucket {
  const MonthBucket({
    required this.month,
    required this.plays,
    required this.hours,
  });
  final int month;
  final int plays;
  final double hours;

  factory MonthBucket.fromJson(Map<String, dynamic> j) => MonthBucket(
    month: (j['month'] ?? 0) as int,
    plays: (j['plays'] ?? 0) as int,
    hours: ((j['hours'] ?? 0) as num).toDouble(),
  );
}

class Stats {
  const Stats({
    required this.label,
    required this.period,
    required this.totals,
    required this.topTracks,
    required this.topArtists,
    required this.topAlbums,
    required this.months,
    required this.daysPlayed,
    required this.longestStreak,
    this.repeatTrack,
    this.firstPlay,
  });
  final String label;
  final String period;
  final StatsTotals totals;
  final List<StatTrack> topTracks;
  final List<StatArtist> topArtists;
  final List<StatAlbum> topAlbums;
  final List<MonthBucket> months;
  final int daysPlayed;
  final int longestStreak;
  final StatTrack? repeatTrack;
  final int? firstPlay;

  factory Stats.fromJson(Map<String, dynamic> j) => Stats(
    label: (j['label'] ?? '') as String,
    period: (j['period'] ?? '') as String,
    totals: StatsTotals.fromJson(j['totals'] as Map<String, dynamic>),
    topTracks: (j['top_tracks'] as List<dynamic>? ?? [])
        .map((e) => StatTrack.fromJson(e as Map<String, dynamic>))
        .toList(growable: false),
    topArtists: (j['top_artists'] as List<dynamic>? ?? [])
        .map((e) => StatArtist.fromJson(e as Map<String, dynamic>))
        .toList(growable: false),
    topAlbums: (j['top_albums'] as List<dynamic>? ?? [])
        .map((e) => StatAlbum.fromJson(e as Map<String, dynamic>))
        .toList(growable: false),
    months: (j['months'] as List<dynamic>? ?? [])
        .map((e) => MonthBucket.fromJson(e as Map<String, dynamic>))
        .toList(growable: false),
    daysPlayed: (j['days_played'] ?? 0) as int,
    longestStreak: (j['longest_streak'] ?? 0) as int,
    repeatTrack: j['repeat_track'] == null
        ? null
        : StatTrack.fromJson(j['repeat_track'] as Map<String, dynamic>),
    firstPlay: j['first_play'] as int?,
  );

  bool get isEmpty => totals.plays == 0;
}

class ArtistSummary {
  const ArtistSummary({
    required this.id,
    required this.name,
    required this.albums,
    required this.tracks,
    required this.plays,
    this.coverAlbum,
    this.hasCover = false,
    this.liked = false,
  });
  final int id;
  final String name;
  final int albums;
  final int tracks;
  final int plays;
  final int? coverAlbum;
  final bool hasCover;
  final bool liked;

  factory ArtistSummary.fromJson(Map<String, dynamic> j) => ArtistSummary(
    id: j['id'] as int,
    name: (j['name'] ?? '') as String,
    albums: (j['albums'] ?? 0) as int,
    tracks: (j['tracks'] ?? 0) as int,
    plays: (j['plays'] ?? 0) as int,
    coverAlbum: j['cover_album'] as int?,
    hasCover: j['has_cover'] == true || j['has_cover'] == 1,
    liked: j['liked'] == true || j['liked'] == 1,
  );
}

class AlbumSummary {
  const AlbumSummary({
    required this.id,
    required this.title,
    required this.artist,
    required this.artistId,
    required this.tracks,
    this.year,
    this.plays = 0,
    this.hasCover = false,
    this.liked = false,
  });
  final int id;
  final String title;
  final String artist;
  final int artistId;
  final int tracks;
  final int? year;
  final int plays;
  final bool hasCover;
  final bool liked;

  factory AlbumSummary.fromJson(Map<String, dynamic> j) => AlbumSummary(
    id: j['id'] as int,
    title: (j['title'] ?? '') as String,
    artist: (j['artist'] ?? '') as String,
    artistId: (j['artist_id'] ?? 0) as int,
    tracks: (j['tracks'] ?? 0) as int,
    year: j['year'] as int?,
    plays: (j['plays'] ?? 0) as int,
    hasCover: j['has_cover'] == true || j['has_cover'] == 1,
    liked: j['liked'] == true || j['liked'] == 1,
  );
}

class AlbumPage {
  const AlbumPage({
    required this.album,
    required this.tracks,
    required this.totalMs,
    required this.plays,
  });
  final AlbumSummary album;
  final List<Track> tracks;
  final int totalMs;
  final int plays;

  factory AlbumPage.fromJson(Map<String, dynamic> j) {
    final album = AlbumSummary.fromJson(j['album'] as Map<String, dynamic>);
    final raw = (j['tracks'] as List<dynamic>? ?? []);
    final tracks = <Track>[];
    for (final e in raw) {
      final m = e as Map<String, dynamic>;
      tracks.add(
        Track(
          id: m['id'] as int,
          title: (m['title'] ?? '') as String,
          artist: album.artist,
          album: album.title,
          albumId: album.id,
          hasCover: album.hasCover,
          year: album.year,
          disc: m['disc'] as int?,
          trackNo: m['track_no'] as int?,
          durationMs: m['duration_ms'] as int?,
          codec: m['codec'] as String?,
          bits: m['bits'] as int?,
          sampleRate: m['sample_rate'] as int?,
          bitrate: m['bitrate'] as int?,
        ),
      );
    }
    return AlbumPage(
      album: album,
      tracks: tracks,
      totalMs: (j['total_ms'] ?? 0) as int,
      plays: (j['plays'] ?? 0) as int,
    );
  }
}

class ArtistPage {
  const ArtistPage({
    required this.artist,
    required this.albums,
    required this.topTracks,
    this.firstYear,
    this.lastYear,
  });
  final ArtistSummary artist;
  final List<AlbumSummary> albums;
  final List<StatTrack> topTracks;
  final int? firstYear;
  final int? lastYear;

  factory ArtistPage.fromJson(Map<String, dynamic> j) => ArtistPage(
    artist: ArtistSummary.fromJson(j['artist'] as Map<String, dynamic>),
    albums: (j['albums'] as List<dynamic>? ?? [])
        .map((e) => AlbumSummary.fromJson(e as Map<String, dynamic>))
        .toList(growable: false),
    topTracks: (j['top_tracks'] as List<dynamic>? ?? [])
        .map((e) => StatTrack.fromJson(e as Map<String, dynamic>))
        .toList(growable: false),
    firstYear: j['first_year'] as int?,
    lastYear: j['last_year'] as int?,
  );
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

/// Adds the session token to every request in one place, so no call site can
/// forget it and quietly fall back to the local user.
class _AuthClient extends http.BaseClient {
  _AuthClient(this._inner, this._token);
  final http.Client _inner;
  final String? Function() _token;

  @override
  Future<http.StreamedResponse> send(http.BaseRequest request) {
    final token = _token();
    if (token != null && token.isNotEmpty) {
      request.headers['Authorization'] = 'Bearer $token';
    }
    return _inner.send(request);
  }
}

class Api {
  Api(this.base) {
    _http = _AuthClient(http.Client(), () => token);
  }
  String base;
  late final http.Client _http;

  /// Session token from the seed phrase, if this device is signed in.
  String? token;

  Future<Map<String, dynamic>> register(String name) async {
    final r = await _http
        .post(
          Uri.parse('$base/api/auth/register'),
          headers: {'content-type': 'application/json'},
          body: jsonEncode({'name': name}),
        )
        .timeout(const Duration(seconds: 20));
    if (r.statusCode != 200) throw ApiException('register → ${r.statusCode}');
    return jsonDecode(r.body) as Map<String, dynamic>;
  }

  Future<Map<String, dynamic>> login(String phrase) async {
    final r = await _http
        .post(
          Uri.parse('$base/api/auth/login'),
          headers: {'content-type': 'application/json'},
          body: jsonEncode({'phrase': phrase}),
        )
        .timeout(const Duration(seconds: 20));
    if (r.statusCode == 401) throw ApiException('фраза не подходит');
    if (r.statusCode != 200) throw ApiException('login → ${r.statusCode}');
    return jsonDecode(r.body) as Map<String, dynamic>;
  }

  /// Whether this device can act as a listener: 'ok', 'needs_auth' (server is
  /// reachable but wants a phrase) or 'unreachable'.
  Future<String> authStatus() async {
    try {
      final r = await _http
          .get(Uri.parse('$base/api/auth/me'))
          .timeout(const Duration(seconds: 6));
      if (r.statusCode == 200) return 'ok';
      if (r.statusCode == 401) return 'needs_auth';
      return 'unreachable';
    } catch (_) {
      return 'unreachable';
    }
  }

  Future<Map<String, dynamic>?> me() async {
    final r = await _http
        .get(Uri.parse('$base/api/auth/me'))
        .timeout(const Duration(seconds: 15));
    if (r.statusCode != 200) return null;
    return jsonDecode(r.body) as Map<String, dynamic>;
  }

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

  Future<SearchResults> search(String query, {int limit = 60}) async {
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
    return SearchResults.fromJson(jsonDecode(r.body) as Map<String, dynamic>);
  }

  Future<Track?> track(int id) async {
    final r = await _http
        .get(Uri.parse('$base/api/tracks/$id'))
        .timeout(const Duration(seconds: 15));
    if (r.statusCode != 200) return null;
    return Track.fromJson(jsonDecode(r.body) as Map<String, dynamic>);
  }

  /// The server indexes tracks, not albums; an album's tracks are found by
  /// searching its title and keeping the matching album id.
  Future<List<Track>> albumTracks(int albumId, String title) async {
    final hit = await search(title, limit: 200);
    return hit.tracks
        .where((t) => t.albumId == albumId)
        .toList(growable: false);
  }

  Future<List<ArtistSummary>> artists() async {
    final r = await _http
        .get(Uri.parse('$base/api/artists?limit=20000'))
        .timeout(const Duration(seconds: 30));
    if (r.statusCode != 200) throw ApiException('artists → ${r.statusCode}');
    return (jsonDecode(r.body) as List<dynamic>)
        .map((e) => ArtistSummary.fromJson(e as Map<String, dynamic>))
        .toList(growable: false);
  }

  Future<ArtistPage> artistPage(int id) async {
    final r = await _http
        .get(Uri.parse('$base/api/artists/$id'))
        .timeout(const Duration(seconds: 20));
    if (r.statusCode != 200) throw ApiException('artist → ${r.statusCode}');
    return ArtistPage.fromJson(jsonDecode(r.body) as Map<String, dynamic>);
  }

  Future<List<AlbumSummary>> albums() async {
    final r = await _http
        .get(Uri.parse('$base/api/albums?limit=20000'))
        .timeout(const Duration(seconds: 30));
    if (r.statusCode != 200) throw ApiException('albums → ${r.statusCode}');
    return (jsonDecode(r.body) as List<dynamic>)
        .map((e) => AlbumSummary.fromJson(e as Map<String, dynamic>))
        .toList(growable: false);
  }

  Future<AlbumPage> albumPage(int id) async {
    final r = await _http
        .get(Uri.parse('$base/api/albums/$id'))
        .timeout(const Duration(seconds: 20));
    if (r.statusCode != 200) throw ApiException('album → ${r.statusCode}');
    return AlbumPage.fromJson(jsonDecode(r.body) as Map<String, dynamic>);
  }

  Future<List<int>> albumLikeIds() async {
    final r = await _http
        .get(Uri.parse('$base/api/likes/albums/ids'))
        .timeout(const Duration(seconds: 15));
    if (r.statusCode != 200) {
      throw ApiException('album likes → ${r.statusCode}');
    }
    return (jsonDecode(r.body) as List<dynamic>).cast<int>();
  }

  Future<void> likeAlbum(int id, {bool on = true}) async {
    final uri = Uri.parse('$base/api/likes/albums/$id');
    final r = on
        ? await _http.post(uri).timeout(const Duration(seconds: 15))
        : await _http.delete(uri).timeout(const Duration(seconds: 15));
    if (r.statusCode != 200) throw ApiException('album like → ${r.statusCode}');
  }

  Future<List<int>> artistLikeIds() async {
    final r = await _http
        .get(Uri.parse('$base/api/likes/artists/ids'))
        .timeout(const Duration(seconds: 15));
    if (r.statusCode != 200) {
      throw ApiException('artist likes → ${r.statusCode}');
    }
    return (jsonDecode(r.body) as List<dynamic>).cast<int>();
  }

  Future<void> likeArtist(int id, {bool on = true}) async {
    final uri = Uri.parse('$base/api/likes/artists/$id');
    final r = on
        ? await _http.post(uri).timeout(const Duration(seconds: 15))
        : await _http.delete(uri).timeout(const Duration(seconds: 15));
    if (r.statusCode != 200) {
      throw ApiException('artist like → ${r.statusCode}');
    }
  }

  Future<Map<String, dynamic>> normalize({bool apply = false}) async {
    final r = await _http
        .post(Uri.parse('$base/api/library/normalize?apply=$apply&limit=2000'))
        .timeout(const Duration(minutes: 2));
    if (r.statusCode != 200) {
      throw ApiException('normalize → ${r.statusCode}');
    }
    return jsonDecode(r.body) as Map<String, dynamic>;
  }

  Future<Stats> stats({String period = 'all'}) async {
    final r = await _http
        .get(Uri.parse('$base/api/stats?period=$period'))
        .timeout(const Duration(seconds: 20));
    if (r.statusCode != 200) throw ApiException('stats → ${r.statusCode}');
    return Stats.fromJson(jsonDecode(r.body) as Map<String, dynamic>);
  }

  Future<Map<String, dynamic>> info() async {
    final r = await _http
        .get(Uri.parse('$base/api/info'))
        .timeout(const Duration(seconds: 20));
    if (r.statusCode != 200) throw ApiException('info → ${r.statusCode}');
    return jsonDecode(r.body) as Map<String, dynamic>;
  }

  Future<List<Track>> likes() async {
    final r = await _http
        .get(Uri.parse('$base/api/likes'))
        .timeout(const Duration(seconds: 15));
    if (r.statusCode != 200) throw ApiException('likes → ${r.statusCode}');
    final list = jsonDecode(r.body) as List<dynamic>;
    return list
        .map((e) => Track.fromJson(e as Map<String, dynamic>))
        .toList(growable: false);
  }

  Future<List<int>> likeIds() async {
    final r = await _http
        .get(Uri.parse('$base/api/likes/ids'))
        .timeout(const Duration(seconds: 15));
    if (r.statusCode != 200) throw ApiException('like ids → ${r.statusCode}');
    return (jsonDecode(r.body) as List<dynamic>).cast<int>();
  }

  Future<void> like(int id) async {
    final r = await _http
        .post(Uri.parse('$base/api/likes/$id'))
        .timeout(const Duration(seconds: 15));
    if (r.statusCode != 200) throw ApiException('like → ${r.statusCode}');
  }

  Future<void> unlike(int id) async {
    final r = await _http
        .delete(Uri.parse('$base/api/likes/$id'))
        .timeout(const Duration(seconds: 15));
    if (r.statusCode != 200) throw ApiException('unlike → ${r.statusCode}');
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
