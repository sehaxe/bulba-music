import 'dart:convert';
import 'dart:io';

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
    this.loudness,
    this.camelot,
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

  /// EBU R128 integrated loudness in LUFS. Null for tracks whose features were
  /// never built; level matching simply skips those.
  final double? loudness;

  /// Camelot code ("8A"), from the key detected at import. Used to choose how
  /// long a blend may be: same code blends for the whole length, a neighbour for
  /// two thirds, a clash for a short one.
  final String? camelot;

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
    loudness: (j['loudness'] as num?)?.toDouble(),
    camelot: j['camelot'] as String?,
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

class MoodResult {
  const MoodResult({required this.phrase, required this.tracks});
  final String? phrase;
  final List<Track> tracks;
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
    required this.skipRate,
    required this.skipDepth,
  });
  final int plays;
  final double hours;
  final int tracks;
  final int artists;
  final int likes;
  final int skips;
  final int repeats;
  final double skipRate;
  final double skipDepth;

  factory StatsTotals.fromJson(Map<String, dynamic> j) => StatsTotals(
    plays: (j['plays'] ?? 0) as int,
    hours: ((j['hours'] ?? 0) as num).toDouble(),
    tracks: (j['tracks'] ?? 0) as int,
    artists: (j['artists'] ?? 0) as int,
    likes: (j['likes'] ?? 0) as int,
    skips: (j['skips'] ?? 0) as int,
    repeats: (j['repeats'] ?? 0) as int,
    skipRate: ((j['skip_rate'] ?? 0) as num).toDouble(),
    skipDepth: ((j['skip_depth'] ?? 0) as num).toDouble(),
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

/// The last thing this listener was playing, with enough context to offer
/// "continue" without a second request.
class PlaybackCursor {
  const PlaybackCursor({
    required this.trackId,
    required this.positionMs,
    required this.updatedAt,
    this.track,
  });
  final int trackId;
  final int positionMs;
  final int updatedAt;
  final Track? track;

  Duration get position => Duration(milliseconds: positionMs);

  /// Worth offering only when there is something left to hear and the point is
  /// not ancient history.
  bool get resumable {
    final t = track;
    if (t == null) return false;
    final total = t.duration.inMilliseconds;
    if (total > 0 && (positionMs < 15000 || positionMs > total - 15000)) {
      return false;
    }
    final age = DateTime.now().millisecondsSinceEpoch - updatedAt;
    return age < const Duration(days: 7).inMilliseconds;
  }
}

/// A group of rows that look like the same artist or album.
class DupGroup {
  const DupGroup({
    required this.kind,
    required this.keep,
    required this.members,
  });
  final String kind;
  final int keep;
  final List<DupMember> members;

  factory DupGroup.fromJson(Map<String, dynamic> j) => DupGroup(
    kind: j['kind'] as String,
    keep: j['keep'] as int,
    members: (j['members'] as List<dynamic>)
        .map((e) => DupMember.fromJson(e as Map<String, dynamic>))
        .toList(),
  );
}

class DupMember {
  const DupMember({required this.id, required this.name, required this.tracks});
  final int id;
  final String name;
  final int tracks;

  factory DupMember.fromJson(Map<String, dynamic> j) => DupMember(
    id: j['id'] as int,
    name: j['name'] as String,
    tracks: (j['tracks'] as num).toInt(),
  );
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

  /// Where the last successful catalogue is kept so the app still opens with no
  /// network. Set once at boot; without it every call goes to the server.
  Directory? cacheDir;

  /// Set at boot so covers and waveforms can prefer the copy that came with a
  /// download. Kept as an untyped callback pair to avoid a circular import.
  String? Function(int albumId)? localCover;
  List<int>? Function(int trackId)? localPeaks;

  Future<List<Track>> tracks({int limit = 2000, int offset = 0}) async {
    final cache = cacheDir == null
        ? null
        : File('${cacheDir!.path}/catalogue-$offset.json');
    try {
      final r = await _http
          .get(Uri.parse('$base/api/tracks?limit=$limit&offset=$offset'))
          .timeout(const Duration(seconds: 10));
      if (r.statusCode != 200) {
        throw ApiException('GET /tracks → ${r.statusCode}');
      }
      // Written before parsing so a decode bug cannot cost the cache.
      if (cache != null) {
        try {
          await cache.parent.create(recursive: true);
          await cache.writeAsString(r.body);
        } catch (_) {}
      }
      return _parseTracks(r.body);
    } catch (e) {
      // Offline is a state, not an error: a downloaded library has to be
      // reachable when the server is on the other side of a dead tunnel.
      if (cache != null && cache.existsSync()) {
        try {
          return _parseTracks(await cache.readAsString());
        } catch (_) {}
      }
      rethrow;
    }
  }

  static List<Track> _parseTracks(String body) {
    final list = jsonDecode(body) as List<dynamic>;
    return list
        .map((e) => Track.fromJson(e as Map<String, dynamic>))
        .toList(growable: false);
  }

  /// What the server is willing to hand out. The cap is applied server-side, so
  /// this only changes the policy — a phone still cannot pull FLAC past it.
  Future<String> setServe(String quality) async {
    final r = await _http
        .post(
          Uri.parse('$base/api/config/serve'),
          headers: {'content-type': 'application/json'},
          body: jsonEncode({'quality': quality}),
        )
        .timeout(const Duration(seconds: 15));
    if (r.statusCode != 200) throw ApiException('serve → ${r.statusCode}');
    return (jsonDecode(r.body) as Map<String, dynamic>)['serve'] as String;
  }

  /// "Не это": a decision, recorded as its own event so the wave excludes the
  /// track instead of merely ranking it lower.
  Future<void> reject(int trackId) async {
    final r = await _http
        .post(
          Uri.parse('$base/api/recs/reject'),
          headers: {'content-type': 'application/json'},
          body: jsonEncode({'track_id': trackId}),
        )
        .timeout(const Duration(seconds: 10));
    if (r.statusCode != 200) throw ApiException('reject → ${r.statusCode}');
  }

  /// Search by feeling: "грустный джаз на ночь". The server answers with the
  /// phrase it understood, so the UI can say what it heard instead of guessing.
  Future<MoodResult> mood(String query, {int limit = 30}) async {
    final r = await _http
        .get(
          Uri.parse(
            '$base/api/recs/mood?q=${Uri.encodeQueryComponent(query)}&limit=$limit',
          ),
        )
        .timeout(const Duration(seconds: 12));
    if (r.statusCode != 200) throw ApiException('mood → ${r.statusCode}');
    final j = jsonDecode(r.body) as Map<String, dynamic>;
    return MoodResult(
      phrase: j['phrase'] as String?,
      tracks: (j['tracks'] as List<dynamic>)
          .map((e) => Track.fromJson(e as Map<String, dynamic>))
          .toList(),
    );
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

  /// The master by default; `quality` asks the server for a derived Opus copy
  /// (opus96 / opus128 / opus160) instead.
  Uri stream(int id, {String? quality}) {
    final q = (quality == null || quality == 'flac') ? '' : '?quality=$quality';
    return Uri.parse('$base/api/tracks/$id/stream$q');
  }

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

  /// Ask for a different set of today's mixes. The cached ones are deleted
  /// server-side, so this is a decision the listener makes, not a poll.
  Future<int> refreshDaily() async {
    final r = await _http
        .post(Uri.parse('$base/api/recs/daily/refresh'))
        .timeout(const Duration(seconds: 30));
    if (r.statusCode != 200) throw ApiException('refresh → ${r.statusCode}');
    return ((jsonDecode(r.body) as Map<String, dynamic>)['mixes'] ?? 0) as int;
  }

  /// Reads a cached body when a request fails. The cache is written from the
  /// same body that was just parsed, so a fallback is always a real answer the
  /// server gave, never a guess.
  Future<String?> _cached(String name) async {
    final dir = cacheDir;
    if (dir == null) return null;
    final f = File('${dir.path}/$name.json');
    if (!f.existsSync()) return null;
    try {
      return await f.readAsString();
    } catch (_) {
      return null;
    }
  }

  Future<void> _cache(String name, String body) async {
    final dir = cacheDir;
    if (dir == null) return;
    try {
      await File('${dir.path}/$name.json').writeAsString(body);
    } catch (_) {}
  }

  Future<List<Mix>> dailyMixes() async {
    try {
      final r = await _http
          .get(Uri.parse('$base/api/recs/daily'))
          .timeout(const Duration(seconds: 30));
      if (r.statusCode != 200) {
        throw ApiException('recs/daily → ${r.statusCode}');
      }
      await _cache('daily', r.body);
      return _parseMixes(r.body);
    } catch (_) {
      final cached = await _cached('daily');
      if (cached == null) rethrow;
      return _parseMixes(cached);
    }
  }

  static List<Mix> _parseMixes(String body) {
    final list = jsonDecode(body) as List<dynamic>;
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
    final local = localPeaks?.call(id);
    if (local != null) {
      _peaks[id] = local;
      return local;
    }
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

  /// Cover URL at a given pixel width. The server renders and caches one file
  /// per width, so a 64 px row thumbnail must not pull the 512 px artwork: the
  /// difference is ~8 KB against ~120 KB, and a home screen shows dozens.
  String? cover(int? albumId, bool hasCover, {int w = 512}) {
    if (!hasCover || albumId == null) return null;
    final local = localCover?.call(albumId);
    if (local != null) return Uri.file(local).toString();
    return '$base/api/albums/$albumId/cover?w=$w';
  }
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

/// A playlist as the server describes it: name, visibility, size, owner.
class Playlist {
  const Playlist({
    required this.id,
    required this.name,
    required this.description,
    required this.isPublic,
    required this.tracks,
    required this.durationMs,
    required this.owner,
    required this.mine,
    required this.liked,
    required this.coverAlbum,
    required this.hasCover,
  });

  final int id;
  final String name;
  final String description;
  final bool isPublic;
  final int tracks;
  final int durationMs;
  final String owner;
  final bool mine;
  final bool liked;
  final int? coverAlbum;
  final bool hasCover;

  factory Playlist.fromJson(Map<String, dynamic> j) => Playlist(
    id: j['id'] as int,
    name: j['name'] as String,
    description: (j['description'] ?? '') as String,
    isPublic: (j['is_public'] ?? false) as bool,
    tracks: (j['tracks'] ?? 0) as int,
    durationMs: (j['duration_ms'] ?? 0) as int,
    owner: (j['owner'] ?? '') as String,
    mine: (j['mine'] ?? false) as bool,
    liked: (j['liked'] ?? false) as bool,
    coverAlbum: j['cover_album'] as int?,
    hasCover: (j['has_cover'] ?? false) as bool,
  );
}

/// One suggestion from the wave, with the sentence that explains it.
class WaveTrack {
  const WaveTrack({
    required this.track,
    required this.reason,
    required this.score,
  });
  final Track track;
  final String reason;
  final double score;

  factory WaveTrack.fromJson(Map<String, dynamic> j) => WaveTrack(
    track: Track.fromJson(j),
    reason: (j['reason'] ?? '') as String,
    score: ((j['score'] ?? 0) as num).toDouble(),
  );
}

class WaveReply {
  const WaveReply({
    required this.tracks,
    required this.seed,
    required this.tasteReady,
  });
  final List<WaveTrack> tracks;
  final int? seed;
  final bool tasteReady;

  factory WaveReply.fromJson(Map<String, dynamic> j) => WaveReply(
    tracks: (j['tracks'] as List<dynamic>)
        .map((e) => WaveTrack.fromJson(e as Map<String, dynamic>))
        .toList(),
    seed: j['seed'] as int?,
    tasteReady: (j['taste_ready'] ?? false) as bool,
  );
}

extension WaveApi on Api {
  Future<WaveReply> wave({
    int? seed,
    double novelty = 0.35,
    int limit = 12,
    Set<int> exclude = const {},
  }) async {
    final params = <String, String>{
      'novelty': novelty.toStringAsFixed(2),
      'limit': '$limit',
      if (seed != null) 'seed': '$seed',
      if (exclude.isNotEmpty) 'exclude': exclude.join(','),
    };
    final uri = Uri.parse('$base/api/recs/wave')
        .replace(queryParameters: params);
    try {
      final r = await _http.get(uri).timeout(const Duration(seconds: 20));
      if (r.statusCode != 200) throw ApiException('wave → ${r.statusCode}');
      // The wave is the one screen that must not go blank on a train: the last
      // batch computed while online is cached per seed.
      await _cache('wave-${seed ?? 'taste'}', r.body);
      return WaveReply.fromJson(jsonDecode(r.body) as Map<String, dynamic>);
    } catch (_) {
      final cached = await _cached('wave-${seed ?? 'taste'}');
      if (cached == null) rethrow;
      return WaveReply.fromJson(jsonDecode(cached) as Map<String, dynamic>);
    }
  }
}

extension PlaylistApi on Api {
  /// Where this listener stopped, if anywhere. The server keeps one row per
  /// user, so the phone and the desktop see the same point.
  Future<PlaybackCursor?> cursor() async {
    final r = await _http
        .get(Uri.parse('$base/api/playback'))
        .timeout(const Duration(seconds: 10));
    if (r.statusCode != 200) return null;
    final j = jsonDecode(r.body);
    if (j is! Map<String, dynamic>) return null;
    final track = j['track'];
    return PlaybackCursor(
      trackId: j['track_id'] as int,
      positionMs: j['position_ms'] as int,
      updatedAt: (j['updated_at'] as int) * 1000,
      track: track is Map<String, dynamic> ? Track.fromJson(track) : null,
    );
  }

  Future<void> putCursor(int trackId, int positionMs) async {
    await _http
        .post(
          Uri.parse('$base/api/playback'),
          headers: {'content-type': 'application/json'},
          body: jsonEncode({'track_id': trackId, 'position_ms': positionMs}),
        )
        .timeout(const Duration(seconds: 10));
  }

  Future<List<DupGroup>> duplicates() async {
    final r = await _http
        .get(Uri.parse('$base/api/library/duplicates'))
        .timeout(const Duration(seconds: 20));
    if (r.statusCode != 200) throw ApiException('duplicates → ${r.statusCode}');
    final list = jsonDecode(r.body) as List<dynamic>;
    return list
        .map((e) => DupGroup.fromJson(e as Map<String, dynamic>))
        .toList();
  }

  Future<void> mergeDuplicates({
    required String kind,
    required int keep,
    required int merge,
  }) async {
    final r = await _http
        .post(
          Uri.parse('$base/api/library/merge'),
          headers: {'content-type': 'application/json'},
          body: jsonEncode({'kind': kind, 'keep': keep, 'merge': merge}),
        )
        .timeout(const Duration(seconds: 60));
    if (r.statusCode != 200) throw ApiException('merge → ${r.statusCode}');
  }

  Future<List<Playlist>> playlists() async {
    final r = await _http
        .get(Uri.parse('$base/api/playlists'))
        .timeout(const Duration(seconds: 15));
    if (r.statusCode != 200) throw ApiException('playlists → ${r.statusCode}');
    return (jsonDecode(r.body) as List<dynamic>)
        .map((e) => Playlist.fromJson(e as Map<String, dynamic>))
        .toList();
  }

  Future<List<Playlist>> likedPlaylists() async {
    final r = await _http
        .get(Uri.parse('$base/api/likes/playlists'))
        .timeout(const Duration(seconds: 15));
    if (r.statusCode != 200) return const [];
    return (jsonDecode(r.body) as List<dynamic>)
        .map((e) => Playlist.fromJson(e as Map<String, dynamic>))
        .toList();
  }

  Future<int> createPlaylist(String name, {bool isPublic = false}) async {
    final r = await _http
        .post(
          Uri.parse('$base/api/playlists'),
          headers: {'content-type': 'application/json'},
          body: jsonEncode({'name': name, 'is_public': isPublic}),
        )
        .timeout(const Duration(seconds: 15));
    if (r.statusCode != 200) throw ApiException('playlist → ${r.statusCode}');
    return (jsonDecode(r.body) as Map<String, dynamic>)['id'] as int;
  }

  Future<void> patchPlaylist(
    int id, {
    String? name,
    String? description,
    bool? isPublic,
  }) async {
    final r = await _http
        .patch(
          Uri.parse('$base/api/playlists/$id'),
          headers: {'content-type': 'application/json'},
          body: jsonEncode({
            'name': ?name,
            'description': ?description,
            'is_public': ?isPublic,
          }),
        )
        .timeout(const Duration(seconds: 15));
    if (r.statusCode != 200) throw ApiException('playlist → ${r.statusCode}');
  }

  Future<void> deletePlaylist(int id) async {
    final r = await _http
        .delete(Uri.parse('$base/api/playlists/$id'))
        .timeout(const Duration(seconds: 15));
    if (r.statusCode != 200) throw ApiException('playlist → ${r.statusCode}');
  }

  Future<List<Track>> playlistTracks(int id) async {
    final r = await _http
        .get(Uri.parse('$base/api/playlists/$id'))
        .timeout(const Duration(seconds: 15));
    if (r.statusCode != 200) throw ApiException('playlist → ${r.statusCode}');
    final j = jsonDecode(r.body) as Map<String, dynamic>;
    return (j['tracks'] as List<dynamic>)
        .map((e) => Track.fromJson(e as Map<String, dynamic>))
        .toList();
  }

  Future<void> addToPlaylist(int id, int trackId) async {
    final r = await _http
        .post(
          Uri.parse('$base/api/playlists/$id/tracks'),
          headers: {'content-type': 'application/json'},
          body: jsonEncode({'track_id': trackId}),
        )
        .timeout(const Duration(seconds: 15));
    if (r.statusCode != 200) throw ApiException('playlist → ${r.statusCode}');
  }

  Future<void> removeFromPlaylist(int id, int trackId) async {
    final r = await _http
        .delete(Uri.parse('$base/api/playlists/$id/tracks/$trackId'))
        .timeout(const Duration(seconds: 15));
    if (r.statusCode != 200) throw ApiException('playlist → ${r.statusCode}');
  }

  /// Persists a drag-and-drop order. The server takes the full list: swapping
  /// one pair would leave the other positions ambiguous after a multi-row drag.
  Future<void> reorderPlaylist(int id, List<int> trackIds) async {
    final r = await _http
        .put(
          Uri.parse('$base/api/playlists/$id/tracks'),
          headers: {'content-type': 'application/json'},
          body: jsonEncode({'track_ids': trackIds}),
        )
        .timeout(const Duration(seconds: 15));
    if (r.statusCode != 200) throw ApiException('playlist → ${r.statusCode}');
  }

  Future<void> likePlaylist(int id, {bool on = true}) async {
    final uri = Uri.parse('$base/api/likes/playlists/$id');
    final r = on
        ? await _http.post(uri).timeout(const Duration(seconds: 15))
        : await _http.delete(uri).timeout(const Duration(seconds: 15));
    if (r.statusCode != 200) {
      throw ApiException('playlist like → ${r.statusCode}');
    }
  }
}
