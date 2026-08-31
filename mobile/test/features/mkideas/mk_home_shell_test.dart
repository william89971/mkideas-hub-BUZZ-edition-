import 'package:buzz/features/home/home_page.dart';
import 'package:buzz/shared/deeplink/deep_link.dart';
import 'package:buzz/shared/deeplink/pending_deep_link_provider.dart';
import 'package:buzz/shared/mkideas/mkideas_provider.dart';
import 'package:buzz/shared/theme/theme.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:shared_preferences/shared_preferences.dart';

void main() {
  testWidgets('keeps Quick Capture available above every five-area tab', (
    tester,
  ) async {
    await tester.pumpWidget(await _home());
    await tester.pumpAndSettle();

    expect(find.byKey(const Key('mk-universal-quick-capture')), findsOneWidget);
    await tester.tap(find.byKey(const Key('mk-universal-quick-capture')));
    await tester.pumpAndSettle();
    expect(find.text('QUICK CAPTURE'), findsOneWidget);
  });

  testWidgets('resolves an MK Ideas deep link into its typed area and record', (
    tester,
  ) async {
    const entityId = '11111111-1111-4111-8111-111111111111';
    final record = MkRecord(
      eventId: 'person-event',
      author: 'human',
      kind: MkEntityType.person.kind,
      createdAt: 1,
      entityId: entityId,
      version: 1,
      schemaVersion: mkSchemaVersion2,
      type: MkEntityType.person,
      payload: const MkPersonData({
        'name': 'Avery Stone',
      }, MkPersonStatus.researching),
    );
    final pending = _FakePendingDeepLinkNotifier(
      const MkIdeasDeepLink(
        community: 'hub.mkideas.test',
        kind: 30803,
        entityId: entityId,
      ),
    );

    await tester.pumpWidget(
      await _home(
        snapshot: MkIdeasSnapshot.fromRecords(records: [record]),
        pending: pending,
      ),
    );
    await tester.pumpAndSettle();

    final title = find.byKey(const Key('mk-entity-title'));
    expect(title, findsOneWidget);
    expect(tester.widget<Text>(title).data, 'Avery Stone');
    expect(pending.consumed, isTrue);
  });

  testWidgets('presents MK live-sync health without blocking the product', (
    tester,
  ) async {
    await tester.pumpWidget(
      await _home(snapshot: MkIdeasSnapshot.empty.withSyncIssue('offline')),
    );
    await tester.pumpAndSettle();

    expect(find.byKey(const Key('mk-sync-health-banner')), findsOneWidget);
    expect(find.text('Live sync paused · Tap to retry'), findsOneWidget);
    expect(find.byKey(const Key('mk-universal-quick-capture')), findsOneWidget);
  });
}

Future<Widget> _home({
  MkIdeasSnapshot snapshot = MkIdeasSnapshot.empty,
  _FakePendingDeepLinkNotifier? pending,
}) async {
  SharedPreferences.setMockInitialValues({});
  final prefs = await SharedPreferences.getInstance();
  return ProviderScope(
    overrides: [
      savedPrefsProvider.overrideWithValue(prefs),
      mkIdeasProvider.overrideWith(() => _FakeMkIdeasNotifier(snapshot)),
      pendingDeepLinkProvider.overrideWith(
        () => pending ?? _FakePendingDeepLinkNotifier(null),
      ),
    ],
    child: MaterialApp(
      theme: AppTheme.light(),
      home: HomePage(settingsPageBuilder: _settingsPage, hasUnreadInbox: false),
    ),
  );
}

Widget _settingsPage(BuildContext context) => const SizedBox.shrink();

class _FakeMkIdeasNotifier extends MkIdeasNotifier {
  _FakeMkIdeasNotifier(this.snapshot);

  final MkIdeasSnapshot snapshot;

  @override
  Future<MkIdeasSnapshot> build() async => snapshot;

  @override
  Future<MkEntityHistoryPage> loadHistoryPage(
    MkEntityCoordinate coordinate, {
    String? cursor,
  }) async => MkEntityHistoryPage(
    records: snapshot.revisionsFor(coordinate.type, coordinate.entityId),
  );
}

class _FakePendingDeepLinkNotifier extends PendingDeepLinkNotifier {
  _FakePendingDeepLinkNotifier(this.initial);

  final BuzzDeepLink? initial;
  bool consumed = false;

  @override
  BuzzDeepLink? build() => initial;

  @override
  Future<DeepLinkCommunityPreparation> prepareCommunity(
    BuzzDeepLink link,
  ) async => DeepLinkCommunityPreparation.ready;

  @override
  void consume() {
    consumed = true;
    state = null;
  }
}
