import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:multicast_dns/multicast_dns.dart';

/// A speaker or TV that can play the stream.
///
/// Two protocols cover almost every device in a house: Chromecast (Google, and
/// most TVs with it built in) and DLNA/UPnP (network receivers, TVs, consoles).
/// Both work the same way — give it a URL and it plays — which is the shape this
/// library already has: the server serves a URL per track.
class CastDevice {
  CastDevice({
    required this.name,
    required this.host,
    required this.port,
    required this.kind,
    this.location = '',
  });

  final String name;
  final String host;
  final int port;
  final CastKind kind;

  /// The device description URL, for DLNA. Chromecast does not use it.
  final String location;

  String get label => kind == CastKind.chromecast ? 'chromecast' : 'dlna';
}

enum CastKind { chromecast, dlna }

/// Discovers both families at once. Results stream in as they answer: a
/// Chromecast replies in well under a second, DLNA can take three.
Stream<CastDevice> discover({Duration window = const Duration(seconds: 4)}) {
  final out = StreamController<CastDevice>();
  final seen = <String>{};
  void add(CastDevice d) {
    if (seen.add('${d.kind}-${d.host}-${d.port}') && !out.isClosed) out.add(d);
  }

  final subs = <StreamSubscription<CastDevice>>[
    _chromecasts().listen(add),
    _dlna().listen(add),
  ];
  Timer(window, () async {
    for (final s in subs) {
      await s.cancel();
    }
    await out.close();
  });
  return out.stream;
}

/// mDNS for `_googlecast._tcp`: name, port (8009) and the device id.
Stream<CastDevice> _chromecasts() {
  final out = StreamController<CastDevice>();
  () async {
    final client = MDnsClient();
    try {
      await client.start();
      await for (final ptr in client.lookup<PtrResourceRecord>(
        ResourceRecordQuery.serverPointer('_googlecast._tcp.local'),
      )) {
        await for (final srv in client.lookup<SrvResourceRecord>(
          ResourceRecordQuery.service(ptr.domainName),
        )) {
          final name = ptr.domainName
              .replaceAll('._googlecast._tcp.local', '')
              .replaceAll('-', ' ');
          if (!out.isClosed) {
            out.add(
              CastDevice(
                name: name,
                host: srv.target.replaceAll(RegExp(r'\.$'), ''),
                port: srv.port,
                kind: CastKind.chromecast,
              ),
            );
          }
        }
      }
    } catch (_) {
      // No mDNS on this network, or the platform forbids multicast: the sheet
      // simply lists DLNA devices instead.
    }
    client.stop();
    if (!out.isClosed) await out.close();
  }();
  return out.stream;
}

/// SSDP: one multicast M-SEARCH; whoever answers is a renderer.
Stream<CastDevice> _dlna() {
  final out = StreamController<CastDevice>();
  RawDatagramSocket? socket;
  StreamSubscription<RawSocketEvent>? sub;
  final seen = <String>{};
  () async {
    try {
      socket = await RawDatagramSocket.bind(InternetAddress.anyIPv4, 0);
    } catch (_) {
      await out.close();
      return;
    }
    socket!.multicastHops = 2;
    const msearch =
        'M-SEARCH * HTTP/1.1\r\n'
        'HOST: 239.255.255.250:1900\r\n'
        'MAN: "ssdp:discover"\r\n'
        'MX: 3\r\n'
        'ST: urn:schemas-upnp-org:device:MediaRenderer:1\r\n'
        '\r\n';
    socket!.send(
      utf8.encode(msearch),
      InternetAddress('239.255.255.250'),
      1900,
    );
    sub = socket!.listen((event) {
      if (event != RawSocketEvent.read) return;
      final dg = socket!.receive();
      if (dg == null) return;
      final text = utf8.decode(dg.data, allowMalformed: true);
      if (!text.startsWith('HTTP/1.1 200')) return;
      final location = RegExp(
        r'^LOCATION:\s*(\S+)',
        caseSensitive: false,
        multiLine: true,
      ).firstMatch(text)?.group(1);
      if (location == null) return;
      final uri = Uri.tryParse(location);
      if (uri == null || !seen.add(location)) return;
      if (!out.isClosed) {
        out.add(
          CastDevice(
            name: uri.pathSegments.isEmpty
                ? dg.address.address
                : '${uri.pathSegments.last} · ${dg.address.address}',
            host: uri.host,
            port: uri.port,
            kind: CastKind.dlna,
            location: location,
          ),
        );
      }
    });
  }();
  Timer(const Duration(seconds: 3), () async {
    await sub?.cancel();
    socket?.close();
    if (!out.isClosed) await out.close();
  });
  return out.stream;
}

/// Anything that can be told to play, pause and stop.
abstract class CastTarget {
  String get name;
  Future<void> load(String url, {String? title, String? artist});
  Future<void> play();
  Future<void> pause();
  Future<void> seek(Duration position);
  Future<void> stop();
  Future<void> close();
}

/// Chromecast: CASTV2, a length-prefixed protobuf envelope carrying JSON
/// payloads over TLS on port 8009. Only what a music player needs: connect, load
/// a URL into the default media receiver, play, pause, seek, stop.
class ChromecastConnection implements CastTarget {
  ChromecastConnection._(this._socket, this.device);

  final Socket _socket;
  final CastDevice device;
  int _requestId = 0;
  String? _mediaSessionId;

  @override
  String get name => device.name;

  static Future<ChromecastConnection> connect(CastDevice device) async {
    final socket = await SecureSocket.connect(
      device.host,
      device.port,
      onBadCertificate: (_) => true, // devices ship self-signed certificates
      timeout: const Duration(seconds: 5),
    );
    final conn = ChromecastConnection._(socket, device);
    conn._listen();
    conn._send('urn:x-cast:com.google.cast.tp.connection', 'receiver-0', {
      'type': 'CONNECT',
    });
    return conn;
  }

  void _listen() {
    var buffer = Uint8List(0);
    _socket.listen((chunk) {
      buffer = Uint8List.fromList([...buffer, ...chunk]);
      while (buffer.length >= 4) {
        final len = ByteData.sublistView(buffer, 0, 4).getUint32(0);
        if (buffer.length < 4 + len) break;
        final opened = decodeCastMessage(buffer.sublist(4, 4 + len));
        buffer = buffer.sublist(4 + len);
        final payload = opened?['payload'];
        if (payload is Map && payload['type'] == 'MEDIA_STATUS') {
          final status = (payload['status'] as List?)?.firstOrNull;
          if (status is Map && status['mediaSessionId'] != null) {
            _mediaSessionId = '${status['mediaSessionId']}';
          }
        }
      }
    }, onError: (_) {});
  }

  void _send(String namespace, String to, Map<String, dynamic> payload) {
    _socket.add(
      encodeCastMessage({
        'source': 'sender-0',
        'destination': to,
        'namespace': namespace,
        'payload': payload,
      }),
    );
  }

  @override
  Future<void> load(String url, {String? title, String? artist}) async {
    // A device that is idle ignores LOAD until the receiver app runs, so the
    // launch comes first and gets a moment to settle.
    _send('urn:x-cast:com.google.cast.receiver', 'receiver-0', {
      'type': 'LAUNCH',
      'appId': 'CC1AD845',
      'requestId': ++_requestId,
    });
    await Future<void>.delayed(const Duration(milliseconds: 900));
    _send('urn:x-cast:com.google.cast.tp.connection', 'receiver-0', {
      'type': 'CONNECT',
    });
    _send('urn:x-cast:com.google.cast.media', 'receiver-0', {
      'type': 'LOAD',
      'requestId': ++_requestId,
      'media': {
        'contentId': url,
        'streamType': 'BUFFERED',
        'contentType': url.contains('.flac') ? 'audio/flac' : 'audio/mpeg',
        'metadata': {
          'metadataType': 0,
          'title': title ?? '',
          'artist': artist ?? '',
        },
      },
      'autoplay': true,
      'currentTime': 0,
    });
  }

  void _media(String type, [Map<String, dynamic> extra = const {}]) {
    _send('urn:x-cast:com.google.cast.media', 'receiver-0', {
      'type': type,
      'requestId': ++_requestId,
      if (_mediaSessionId != null) 'mediaSessionId': _mediaSessionId,
      ...extra,
    });
  }

  @override
  Future<void> play() async => _media('PLAY');

  @override
  Future<void> pause() async => _media('PAUSE');

  @override
  Future<void> seek(Duration position) async =>
      _media('SEEK', {'currentTime': position.inMilliseconds / 1000});

  @override
  Future<void> stop() async {
    _media('STOP');
    _send('urn:x-cast:com.google.cast.receiver', 'receiver-0', {
      'type': 'STOP',
      'requestId': ++_requestId,
    });
  }

  @override
  Future<void> close() async {
    try {
      await _socket.flush();
      await _socket.close();
    } catch (_) {}
  }
}

/// DLNA/UPnP renderer. The device description says where its AVTransport
/// control endpoint lives; everything after that is SOAP.
class DlnaConnection implements CastTarget {
  DlnaConnection._(this.device, this._controlUrl, this._http);

  final CastDevice device;
  final String _controlUrl;
  final HttpClient _http;

  @override
  String get name => device.name;

  static Future<DlnaConnection> connect(CastDevice device) async {
    final http = HttpClient()..connectionTimeout = const Duration(seconds: 5);
    final req = await http.getUrl(Uri.parse(device.location));
    final res = await req.close();
    final xml = await res.transform(utf8.decoder).join();
    // First AVTransport service in the description. Every renderer has exactly
    // one, and looking for it by name beats guessing the path.
    final service = RegExp(
      r'<service>(?:(?!</service>).)*?AVTransport(?:(?!</service>).)*?</service>',
      caseSensitive: false,
      dotAll: true,
    ).firstMatch(xml)?.group(0);
    if (service == null) {
      throw StateError('нет AVTransport у ${device.name}');
    }
    final control = RegExp(
      r'<controlURL>(.*?)</controlURL>',
      caseSensitive: false,
      dotAll: true,
    ).firstMatch(service)?.group(1)?.trim();
    if (control == null) throw StateError('нет controlURL у ${device.name}');
    final base = Uri.parse(device.location);
    final controlUrl = control.startsWith('http')
        ? control
        : base.resolve(control).toString();
    return DlnaConnection._(device, controlUrl, http);
  }

  Future<void> _soap(String action, String body) async {
    final envelope =
        '<?xml version="1.0" encoding="utf-8"?>'
        '<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" '
        's:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">'
        '<s:Body><u:$action xmlns:u="urn:schemas-upnp-org:service:AVTransport:1">'
        '$body</u:$action></s:Body></s:Envelope>';
    final req = await _http.postUrl(Uri.parse(_controlUrl));
    req.headers.contentType = ContentType('text', 'xml', charset: 'utf-8');
    req.headers.set(
      'SOAPACTION',
      '"urn:schemas-upnp-org:service:AVTransport:1#$action"',
    );
    req.add(utf8.encode(envelope));
    final res = await req.close();
    await res.drain<void>();
    if (res.statusCode >= 400) {
      throw StateError('$action → ${res.statusCode}');
    }
  }

  @override
  Future<void> load(String url, {String? title, String? artist}) async {
    await _soap(
      'SetAVTransportURI',
      '<InstanceID>0</InstanceID>'
          '<CurrentURI>${const HtmlEscape().convert(url)}</CurrentURI>'
          '<CurrentURIMetaData>'
          '${const HtmlEscape().convert(_didl(url, title, artist))}'
          '</CurrentURIMetaData>',
    );
    await play();
  }

  static String _didl(String url, String? title, String? artist) =>
      '<DIDL-Lite xmlns="urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/" '
      'xmlns:dc="http://purl.org/dc/elements/1.1/" '
      'xmlns:upnp="urn:schemas-upnp-org:metadata-1-0/upnp/">'
      '<item id="1" parentID="0" restricted="1">'
      '<dc:title>${const HtmlEscape().convert(title ?? '')}</dc:title>'
      '<dc:creator>${const HtmlEscape().convert(artist ?? '')}</dc:creator>'
      '<upnp:class>object.item.audioItem.musicTrack</upnp:class>'
      '<res protocolInfo="http-get:*:audio/mpeg:*">${const HtmlEscape().convert(url)}</res>'
      '</item></DIDL-Lite>';

  @override
  Future<void> play() async =>
      _soap('Play', '<InstanceID>0</InstanceID><Speed>1</Speed>');

  @override
  Future<void> pause() async => _soap('Pause', '<InstanceID>0</InstanceID>');

  @override
  Future<void> stop() async => _soap('Stop', '<InstanceID>0</InstanceID>');

  @override
  Future<void> seek(Duration position) async => _soap(
    'Seek',
    '<InstanceID>0</InstanceID><Unit>REL_TIME</Unit>'
        '<Target>${_hms(position)}</Target>',
  );

  static String _hms(Duration d) {
    final h = d.inHours.toString().padLeft(2, '0');
    final m = (d.inMinutes % 60).toString().padLeft(2, '0');
    final s = (d.inSeconds % 60).toString().padLeft(2, '0');
    return '$h:$m:$s';
  }

  @override
  Future<void> close() async => _http.close(force: true);
}

/// CASTV2 envelope, hand-rolled: five string fields, one enum, one payload.
///
/// Public because protobuf framing is the one part of casting that can be
/// checked without a device: a wrong length prefix is silently ignored by the
/// receiver, which looks exactly like "the TV does not answer".
Uint8List encodeCastMessage(Map<String, dynamic> m) {
  final out = BytesBuilder();
  void varint(int value) {
    var v = value;
    while (v >= 0x80) {
      out.addByte((v & 0x7f) | 0x80);
      v >>= 7;
    }
    out.addByte(v);
  }

  void str(int field, String value) {
    if (value.isEmpty) return;
    final bytes = utf8.encode(value);
    out.addByte(field << 3 | 2);
    varint(bytes.length);
    out.add(bytes);
  }

  out.addByte(1 << 3); // protocol_version = 1
  out.addByte(1);
  str(2, m['source'] as String);
  str(3, m['destination'] as String);
  str(4, m['namespace'] as String);
  out.addByte(5 << 3); // payload_type = STRING
  out.addByte(0);
  str(6, jsonEncode(m['payload']));
  final body = out.toBytes();
  final framed = BytesBuilder();
  final header = ByteData(4)..setUint32(0, body.length);
  framed.add(header.buffer.asUint8List());
  framed.add(body);
  return framed.toBytes();
}

Map<String, dynamic>? decodeCastMessage(Uint8List body) {
  var i = 0;
  final fields = <int, dynamic>{};
  int readVarint() {
    var shift = 0;
    var value = 0;
    while (i < body.length) {
      final b = body[i++];
      value |= (b & 0x7f) << shift;
      if (b & 0x80 == 0) break;
      shift += 7;
    }
    return value;
  }

  while (i < body.length) {
    final key = readVarint();
    final field = key >> 3;
    final wire = key & 7;
    if (wire == 0) {
      fields[field] = readVarint();
    } else if (wire == 2) {
      final len = readVarint();
      if (i + len > body.length) break;
      fields[field] = utf8.decode(
        body.sublist(i, i + len),
        allowMalformed: true,
      );
      i += len;
    } else {
      break;
    }
  }
  final payload = fields[6];
  if (payload is! String) return null;
  try {
    return {
      'source': fields[2],
      'destination': fields[3],
      'namespace': fields[4],
      'payload': jsonDecode(payload),
    };
  } catch (_) {
    return null;
  }
}
