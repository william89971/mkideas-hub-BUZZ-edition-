import 'package:buzz/features/profile/profile_provider.dart';
import 'package:buzz/features/search/recent_searches_provider.dart';
import 'package:buzz/features/search/search_page.dart';
import 'package:buzz/features/search/search_provider.dart';
import 'package:buzz/shared/mkideas/mkideas_provider.dart';
import 'package:buzz/shared/mkideas/mkideas_search.dart';
import 'package:buzz/shared/profile/user_profile.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import '../../helpers/widget_helpers.dart';

void main() {
  testWidgets('opens a typed MK Ideas result through the shell callback', (
    tester,
  ) async {
    final record = MkRecord(
      eventId: 'person-event',
      author: 'human',
      kind: MkEntityType.person.kind,
      createdAt: 1,
      entityId: '11111111-1111-4111-8111-111111111111',
      version: 1,
      schemaVersion: mkSchemaVersion2,
      type: MkEntityType.person,
      payload: const MkPersonData({
        'name': 'Avery Stone',
        'organization': 'Northstar',
      }, MkPersonStatus.researching),
    );
    final result = MkIdeasSearchResult(
      resultType: MkIdeasSearchResultType.entity,
      entityType: record.type,
      entityId: record.entityId,
      community: 'hub.mkideas.test',
      title: record.title,
      summary: 'Northstar',
      createdAt: record.createdAt,
      sourceEventId: record.eventId,
      record: record,
    );
    MkIdeasSearchResult? selected;
    final recent = _FakeRecentSearchesNotifier();

    await tester.pumpWidget(
      WidgetHelpers.testable(
        overrides: [
          searchProvider.overrideWith(
            () => _FakeSearchNotifier(
              SearchState(query: 'Avery', mkIdeasResults: [result]),
            ),
          ),
          recentSearchesProvider.overrideWith(() => recent),
          profileProvider.overrideWith(_FakeProfileNotifier.new),
        ],
        child: SearchPage(onMkIdeasResultSelected: (value) => selected = value),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('MK Ideas'), findsOneWidget);
    expect(find.text('Avery Stone'), findsOneWidget);
    await tester.tap(
      find.byKey(const ValueKey('search-mkideas-row-person-event')),
    );
    await tester.pump();

    expect(selected, same(result));
    expect(recent.searches, ['Avery']);
  });
}

class _FakeSearchNotifier extends SearchNotifier {
  _FakeSearchNotifier(this.initialState);

  final SearchState initialState;

  @override
  SearchState build() => initialState;
}

class _FakeRecentSearchesNotifier extends RecentSearchesNotifier {
  List<String> get searches => state;

  @override
  List<String> build() => const [];

  @override
  void record(String query) => state = [query];
}

class _FakeProfileNotifier extends ProfileNotifier {
  @override
  Future<UserProfile?> build() async =>
      const UserProfile(pubkey: 'test', displayName: 'Test');
}
