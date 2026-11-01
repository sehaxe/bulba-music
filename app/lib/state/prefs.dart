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

  int get crossfade => _sp.getInt('crossfade_seconds') ?? 0;
  Future<void> setCrossfade(int v) => _sp.setInt('crossfade_seconds', v);

  String? get lastTab => _sp.getString('last_tab');
  Future<void> setLastTab(String v) => _sp.setString('last_tab', v);

  static ThemeMode modeFrom(String raw) => switch (raw) {
    'light' => ThemeMode.light,
    'system' => ThemeMode.system,
    _ => ThemeMode.dark,
  };
}
