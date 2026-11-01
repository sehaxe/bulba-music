import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:http/http.dart' as http;

/// A colour lifted from the current cover. Cached per URL so switching tracks
/// does not re-decode the same art.
class AccentCache {
  static final Map<String, Color> _cache = {};
  static final Map<String, Future<Color>> _inflight = {};

  static Color fallback = const Color(0xFFC2410C);

  static Future<Color> of(String? url) {
    if (url == null || url.isEmpty) return Future.value(fallback);
    final hit = _cache[url];
    if (hit != null) return Future.value(hit);
    return _inflight.putIfAbsent(url, () async {
      try {
        final color = await _compute(url);
        _cache[url] = color;
        return color;
      } catch (_) {
        return fallback;
      } finally {
        _inflight.remove(url);
      }
    });
  }

  static Future<Color> _compute(String url) async {
    final res = await http
        .get(Uri.parse(url))
        .timeout(const Duration(seconds: 8));
    if (res.statusCode != 200) return fallback;
    final codec = await ui.instantiateImageCodec(
      res.bodyBytes,
      targetWidth: 32,
      targetHeight: 32,
    );
    final frame = await codec.getNextFrame();
    final data = await frame.image.toByteData(
      format: ui.ImageByteFormat.rawRgba,
    );
    if (data == null) return fallback;
    return _dominant(data.buffer.asUint8List());
  }

  /// Average in a perceptual-ish way: weight saturated, mid-lightness pixels
  /// more, and reject near-white/near-black, which are usually not the mood.
  static Color _dominant(Uint8List rgba) {
    var r = 0.0, g = 0.0, b = 0.0, wsum = 0.0;
    for (var i = 0; i + 3 < rgba.length; i += 4) {
      final a = rgba[i + 3] / 255.0;
      if (a < 0.5) continue;
      final cr = rgba[i] / 255.0,
          cg = rgba[i + 1] / 255.0,
          cb = rgba[i + 2] / 255.0;
      final max = cr > cg ? (cr > cb ? cr : cb) : (cg > cb ? cg : cb);
      final min = cr < cg ? (cr < cb ? cr : cb) : (cg < cb ? cg : cb);
      final sat = max == 0 ? 0 : (max - min) / max;
      final lum = 0.2126 * cr + 0.7152 * cg + 0.0722 * cb;
      if (lum < 0.06 || lum > 0.94) continue;
      final w = 0.15 + sat * sat * 1.6;
      r += cr * w;
      g += cg * w;
      b += cb * w;
      wsum += w;
    }
    if (wsum == 0) return fallback;
    var c = Color.from(
      alpha: 1,
      red: r / wsum,
      green: g / wsum,
      blue: b / wsum,
    );
    c = _normalise(c);
    return c;
  }

  /// Push the hue into a usable accent range: enough chroma to read, enough
  /// lightness to stay legible on both paper and ink.
  static Color _normalise(Color c) {
    final h = HSLColor.fromColor(c);
    return h
        .withSaturation(h.saturation.clamp(0.35, 0.9))
        .withLightness(h.lightness.clamp(0.42, 0.62))
        .toColor();
  }
}
