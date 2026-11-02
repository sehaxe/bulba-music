import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:shared_preferences/shared_preferences.dart';

/// One saved listener on this device.
///
/// The phrase is the only credential the server accepts, so a switcher that
/// cannot store it is not a switcher — it is a "type twelve words again" screen.
/// The phrase already lives on the owner's paper; storing it next to the token
/// it mints changes nothing about what an attacker with this device can do.
class Profile {
  const Profile({required this.name, required this.phrase, this.userId});

  final String name;
  final String phrase;
  final int? userId;

  Map<String, dynamic> toJson() => {
    'name': name,
    'phrase': phrase,
    if (userId != null) 'user_id': userId,
  };

  static Profile? fromJson(Map<String, dynamic> j) {
    final phrase = j['phrase'];
    if (phrase is! String || phrase.isEmpty) return null;
    return Profile(
      name: (j['name'] as String?) ?? 'слушатель',
      phrase: phrase,
      userId: j['user_id'] as int?,
    );
  }

  Profile copyWith({String? name, int? userId}) => Profile(
    name: name ?? this.name,
    phrase: phrase,
    userId: userId ?? this.userId,
  );
}

/// The list of profiles saved on this device, and which one is active.
class Profiles extends ChangeNotifier {
  Profiles._(this._sp);

  final SharedPreferences _sp;
  static const _key = 'profiles';
  static const _activeKey = 'active_profile';

  static Future<Profiles> load() async {
    final p = Profiles._(await SharedPreferences.getInstance());
    return p;
  }

  List<Profile> get all {
    final raw = _sp.getString(_key);
    if (raw == null) return const [];
    try {
      final list = jsonDecode(raw) as List<dynamic>;
      return list
          .map((e) => Profile.fromJson(e as Map<String, dynamic>))
          .whereType<Profile>()
          .toList();
    } catch (_) {
      return const [];
    }
  }

  String? get activePhrase => _sp.getString(_activeKey);

  Profile? get active {
    final phrase = activePhrase;
    if (phrase == null) return null;
    for (final p in all) {
      if (p.phrase == phrase) return p;
    }
    return null;
  }

  Future<void> save(Profile profile) async {
    final list = all.where((p) => p.phrase != profile.phrase).toList()
      ..add(profile);
    await _sp.setString(_key, jsonEncode(list.map((e) => e.toJson()).toList()));
    await setActive(profile.phrase);
    notifyListeners();
  }

  Future<void> setActive(String phrase) async {
    await _sp.setString(_activeKey, phrase);
    notifyListeners();
  }

  Future<void> remove(String phrase) async {
    final list = all.where((p) => p.phrase != phrase).toList();
    await _sp.setString(_key, jsonEncode(list.map((e) => e.toJson()).toList()));
    if (activePhrase == phrase) await _sp.remove(_activeKey);
    notifyListeners();
  }
}
