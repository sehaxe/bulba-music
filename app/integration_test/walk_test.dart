import 'package:bulba_music/main.dart' as app;
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';

/// Walks the app the way a person does: tabs, a detail screen, back, search.
///
/// The point is not pixel assertions — it is that every screen can be entered
/// and left without an exception and without losing where you were, which is
/// the class of bug a screenshot cannot catch.
void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  Future<void> settle(WidgetTester tester, [int ms = 1200]) async {
    for (var i = 0; i < ms ~/ 50; i++) {
      await tester.pump(const Duration(milliseconds: 50));
    }
  }

  /// Every visible label, so a failure says what was on screen instead of just
  /// "no element".
  String screen(WidgetTester tester) {
    final texts = tester
        .widgetList<Text>(find.byType(Text))
        .map((t) => t.data)
        .whereType<String>()
        .take(24)
        .join(' | ');
    return texts;
  }

  testWidgets('every tab opens and closes, and a detail returns to its tab', (
    tester,
  ) async {
    app.main();
    await settle(tester, 5000);

    final rail = ['библиотека', 'поиск', 'любимое', 'волна'];
    var visited = 0;
    for (final label in rail) {
      final matches = find.text(label);
      if (matches.evaluate().isEmpty) continue;
      visited++;
      await tester.tap(matches.first);
      await settle(tester, 1000);
      expect(
        tester.takeException(),
        isNull,
        reason: 'tab «$label» threw; screen was: ${screen(tester)}',
      );
    }
    expect(
      visited,
      greaterThan(0),
      reason: 'no tab was reachable: ${screen(tester)}',
    );

    // Library: open the first row, then go back. The tab must still be there.
    final library = find.text('библиотека');
    if (library.evaluate().isNotEmpty) {
      await tester.tap(library.first);
      await settle(tester, 1000);
    }
    final rows = find.byType(ListTile);
    if (rows.evaluate().isNotEmpty) {
      await tester.tap(rows.first);
      await settle(tester, 1400);
      expect(
        tester.takeException(),
        isNull,
        reason: 'opening the first row threw; screen was: ${screen(tester)}',
      );
      final back = find.byIcon(Icons.arrow_back_rounded);
      if (back.evaluate().isNotEmpty) {
        await tester.tap(back.first);
        await settle(tester, 1000);
      }
      expect(
        find.text('библиотека').evaluate().isNotEmpty,
        isTrue,
        reason: 'back must land on the library; screen was: ${screen(tester)}',
      );
    }
    expect(tester.takeException(), isNull);
  });

  testWidgets('the system back button walks the app instead of dropping out', (
    tester,
  ) async {
    app.main();
    await settle(tester, 5000);

    // Search is a leaf tab: back from it must land on home, and back from home
    // is the only place the app is allowed to leave.
    final search = find.text('поиск');
    if (search.evaluate().isNotEmpty) {
      await tester.tap(search.first);
      await settle(tester, 900);
      await tester.binding.handlePopRoute();
      await settle(tester, 900);
      expect(
        find.text('главная').evaluate().isNotEmpty ||
            find.text('часто слушаешь').evaluate().isNotEmpty,
        isTrue,
        reason:
            'back from a tab must return home; screen was: ${screen(tester)}',
      );
    }

    // A detail opened from a tab: back pops exactly one place.
    final library = find.text('библиотека');
    if (library.evaluate().isNotEmpty) {
      await tester.tap(library.first);
      await settle(tester, 900);
      final rows = find.byType(ListTile);
      if (rows.evaluate().isNotEmpty) {
        await tester.tap(rows.first);
        await settle(tester, 1200);
        final opened = screen(tester);
        await tester.binding.handlePopRoute();
        await settle(tester, 1000);
        expect(
          find.text('библиотека').evaluate().isNotEmpty,
          isTrue,
          reason:
              'back from «$opened» must land on the library; '
              'screen was: ${screen(tester)}',
        );
      }
    }
    expect(tester.takeException(), isNull);
  });
}
