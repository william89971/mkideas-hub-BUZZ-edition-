import 'package:buzz/features/mkideas/mk_person_detail_page.dart';
import 'package:buzz/shared/mkideas/mkideas_provider.dart';
import 'package:flutter_test/flutter_test.dart';

import '../../helpers/widget_helpers.dart';

void main() {
  testWidgets('shows DNC, linked work, contextual agents, and history', (
    tester,
  ) async {
    final person = _person();
    final interview = MkRecord(
      eventId: 'interview-event',
      author: 'partner',
      kind: MkEntityType.interview.kind,
      createdAt: 2,
      entityId: 'interview-id',
      version: 1,
      schemaVersion: 2,
      type: MkEntityType.interview,
      payload: MkInterviewData({
        'title': 'Avery interview',
        'guest_id': person.entityId,
      }, MkInterviewStatus.planning),
    );
    final notifier = _FakeMkIdeasNotifier(
      MkIdeasSnapshot.fromRecords(records: [person, interview]),
    );

    await tester.pumpWidget(
      WidgetHelpers.testable(
        overrides: [mkIdeasProvider.overrideWith(() => notifier)],
        child: MkPersonDetailPage(person: person),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.textContaining('DO NOT CONTACT'), findsOneWidget);
    expect(find.text('Guest Researcher'), findsOneWidget);
    await tester.scrollUntilVisible(find.text('Outreach Drafter'), 250);
    expect(find.text('Outreach Drafter'), findsOneWidget);
    await tester.scrollUntilVisible(find.text('Interview Producer'), 250);
    expect(find.text('Interview Producer'), findsOneWidget);
    await tester.scrollUntilVisible(find.text('Avery interview'), 300);
    expect(find.text('Avery interview'), findsOneWidget);
    await tester.scrollUntilVisible(find.textContaining('Version 1'), 300);
    expect(find.textContaining('Signed by'), findsOneWidget);
  });
}

MkRecord _person() => const MkRecord(
  eventId: 'person-event',
  author: 'human-partner',
  kind: 30803,
  createdAt: 1,
  entityId: 'person-id',
  version: 1,
  schemaVersion: 2,
  type: MkEntityType.person,
  payload: MkPersonData({
    'name': 'Avery Stone',
    'organization': 'Northstar',
    'title': 'Founder',
    'why_now': 'New community research practice',
    'do_not_contact': true,
  }, MkPersonStatus.researching),
);

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
