import 'package:buzz/features/mkideas/mk_today_model.dart';
import 'package:buzz/shared/mkideas/mkideas_provider.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('ranks human judgment before blocked and agent outcomes', () {
    final snapshot = MkIdeasSnapshot.fromRecords(
      records: [
        _record(
          type: MkEntityType.task,
          payload: const MkTaskData({
            'title': 'Blocked task',
          }, MkTaskStatus.blocked),
        ),
      ],
      proposals: [
        const MkProposal(
          eventId: 'proposal-event',
          proposalId: 'proposal-id',
          targetId: 'target-id',
          targetKind: 30803,
          agent: 'Guest Researcher',
          summary: 'Research awaits review',
          provenance: ['Fixture source'],
          clips: [],
          createdAt: 10,
          schemaVersion: 2,
          targetEventId: 'target-event',
          targetVersion: 1,
        ),
      ],
      agentRuns: const [
        MkAgentRun(
          eventId: 'run-event',
          createdAt: 9,
          runId: 'run-id',
          persona: MkAgentPersona.outreachDrafter,
          status: MkAgentRunStatus.failed,
          attempt: 1,
          errorMessage: 'DNC stopped the draft.',
        ),
      ],
    );

    final items = buildMkTodayItems(snapshot, now: DateTime.utc(2026, 8, 30));

    expect(items.first.group, MkTodayGroup.judgment);
    expect(items[1].title, 'Blocked task');
    expect(items.last.group, MkTodayGroup.agentOutcomes);
  });

  test('deduplicates blocked and overdue signals for one entity', () {
    final task = _record(
      type: MkEntityType.task,
      payload: const MkTaskData({
        'title': 'Review captions',
        'due_at': '2026-08-29T12:00:00Z',
      }, MkTaskStatus.blocked),
    );

    final items = buildMkTodayItems(
      MkIdeasSnapshot.fromRecords(records: [task]),
      now: DateTime.utc(2026, 8, 30),
    );

    expect(items, hasLength(1));
    expect(items.single.detail, contains('blocked'));
  });
}

MkRecord _record({required MkEntityType type, required MkEntityData payload}) =>
    MkRecord(
      eventId: 'event-${type.kind}',
      author: 'human',
      kind: type.kind,
      createdAt: 1,
      entityId: 'entity-${type.kind}',
      version: 1,
      schemaVersion: 2,
      type: type,
      payload: payload,
    );
