import 'package:flutter/material.dart';
import 'package:shared_preferences/shared_preferences.dart';

/// Small device-local state: nothing here belongs on the server, because it
/// describes this screen, not the library.
class Prefs {
  Prefs(this._sp);

  final SharedPreferences _sp;

  static Future<Prefs> load() async =>
      Prefs(await SharedPreferences.getInstance());

  bool get onboardingSeen => _sp.getBool('onboarding_seen') ?? false;
  Future<void> setOnboardingSeen() => _sp.setBool('onboarding_seen', true);

  String get themeMode => _sp.getString('theme_mode') ?? 'dark';
  Future<void> setThemeMode(String v) => _sp.setString('theme_mode', v);

  double get volume => _sp.getDouble('volume') ?? 1.0;
  Future<void> setVolume(double v) => _sp.setDouble('volume', v);

  /// Eight seconds by default: a library played end to end is a set, and a set
  /// does not stop between songs. Zero stays zero once chosen — the slider is the
  /// only thing that can say "I want hard cuts".
  int get crossfade => _sp.getInt('crossfade_seconds') ?? 8;
  Future<void> setCrossfade(int v) => _sp.setInt('crossfade_seconds', v);

  bool get replayGain => _sp.getBool('replay_gain') ?? true;
  Future<void> setReplayGain(bool v) => _sp.setBool('replay_gain', v);

  String get eq => _sp.getString('eq_preset') ?? 'flat';
  Future<void> setEq(String v) => _sp.setString('eq_preset', v);

  String get quality => _sp.getString('quality') ?? 'flac';
  Future<void> setQuality(String q) => _sp.setString('quality', q);

  String? get server => _sp.getString('server');
  Future<void> setServer(String base) => _sp.setString('server', base);

  String? get token => _sp.getString('auth_token');
  Future<void> setToken(String? token) async {
    if (token == null) {
      await _sp.remove('auth_token');
    } else {
      await _sp.setString('auth_token', token);
    }
  }

  String? get lastTab => _sp.getString('last_tab');
  Future<void> setLastTab(String v) => _sp.setString('last_tab', v);

  static ThemeMode modeFrom(String raw) => switch (raw) {
    'light' => ThemeMode.light,
    'system' => ThemeMode.system,
    _ => ThemeMode.dark,
  };
}
