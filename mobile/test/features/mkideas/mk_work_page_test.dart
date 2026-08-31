import 'package:buzz/features/mkideas/mk_work_page.dart';
import 'package:buzz/shared/mkideas/mkideas_provider.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import '../../helpers/widget_helpers.dart';

void main() {
  testWidgets('renders typed work metrics and filters the operating board', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(430, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final notifier = _FakeMkIdeasNotifier(
      MkIdeasSnapshot.fromRecords(
        records: [
          _task('task-1', 'Book studio', MkTaskStatus.inProgress),
          _project('project-1', 'Fall interview season'),
          _meeting('meeting-1', 'Weekly editorial'),
        ],
      ),
    );

    await tester.pumpWidget(
      WidgetHelpers.testable(
        overrides: [mkIdeasProvider.overrideWith(() => notifier)],
        child: MkWorkPage(onSearch: () {}),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('Work'), findsOneWidget);
    expect(find.text('Open tasks'), findsOneWidget);
    expect(find.text('Active projects'), findsOneWidget);
    expect(find.text('Meetings'), findsOneWidget);
    expect(find.text('Book studio'), findsOneWidget);
    expect(find.text('Fall interview season'), findsOneWidget);
    expect(find.text('Weekly editorial'), findsOneWidget);

    await tester.enterText(find.byKey(const Key('mk-work-filter')), 'studio');
    await tester.pump();

    expect(find.text('Book studio'), findsOneWidget);
    expect(find.text('Fall interview season'), findsNothing);
    expect(find.text('Weekly editorial'), findsNothing);
  });

  testWidgets('offers Quick Capture from the Work create menu', (tester) async {
    final notifier = _FakeMkIdeasNotifier(MkIdeasSnapshot.empty);
    await tester.pumpWidget(
      WidgetHelpers.testable(
        overrides: [mkIdeasProvider.overrideWith(() => notifier)],
        child: MkWorkPage(onSearch: () {}),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.byKey(const Key('mk-work-create-menu')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Task').last);
    await tester.pumpAndSettle();

    expect(find.text('QUICK CAPTURE'), findsOneWidget);
    expect(find.text('Task'), findsWidgets);
  });

  testWidgets('sends typed status changes through the provider', (
    tester,
  ) async {
    final task = _task('task-2', 'Review transcript', MkTaskStatus.review);
    final notifier = _FakeMkIdeasNotifier(
      MkIdeasSnapshot.fromRecords(records: [task]),
    );
    await tester.pumpWidget(
      WidgetHelpers.testable(
        overrides: [mkIdeasProvider.overrideWith(() => notifier)],
        child: MkWorkPage(onSearch: () {}),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.byKey(const Key('mk-work-status-task-2')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('done'));
    await tester.pumpAndSettle();

    expect(notifier.updatedRecord, same(task));
    expect(notifier.updatedStatus, MkTaskStatus.done);
  });
}

MkRecord _task(String id, String title, MkTaskStatus status) => MkRecord(
  eventId: 'event-$id',
  author: 'author',
  kind: MkEntityType.task.kind,
  createdAt: 3,
  entityId: id,
  version: 1,
  schemaVersion: mkSchemaVersion2,
  type: MkEntityType.task,
  payload: MkTaskData({'title': title}, status),
);

MkRecord _project(String id, String title) => MkRecord(
  eventId: 'event-$id',
  author: 'author',
  kind: MkEntityType.operationalProject.kind,
  createdAt: 2,
  entityId: id,
  version: 1,
  schemaVersion: mkSchemaVersion2,
  type: MkEntityType.operationalProject,
  payload: MkProjectData({'title': title}, MkProjectStatus.active),
);

MkRecord _meeting(String id, String title) => MkRecord(
  eventId: 'event-$id',
  author: 'author',
  kind: MkEntityType.meeting.kind,
  createdAt: 1,
  entityId: id,
  version: 1,
  schemaVersion: mkSchemaVersion2,
  type: MkEntityType.meeting,
  payload: MkMeetingData({'title': title}, MkMeetingStatus.planned),
);

class _FakeMkIdeasNotifier extends MkIdeasNotifier {
  _FakeMkIdeasNotifier(this.snapshot);

  final MkIdeasSnapshot snapshot;
  MkRecord? updatedRecord;
  MkStatusValue? updatedStatus;

  @override
  Future<MkIdeasSnapshot> build() async => snapshot;

  @override
  Future<void> updateStatus(MkRecord record, MkStatusValue status) async {
    updatedRecord = record;
    updatedStatus = status;
  }
}
