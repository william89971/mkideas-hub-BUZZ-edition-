import 'dart:convert';

import 'package:buzz/shared/mkideas/mkideas_provider.dart';
import 'package:buzz/shared/relay/nostr_models.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  group('NIP-MK v2 models', () {
    final cases = <({MkEntityType type, MkStatusValue status, Type payload})>[
      (
        type: MkEntityType.goal,
        status: MkGoalStatus.active,
        payload: MkGoalData,
      ),
      (
        type: MkEntityType.operationalProject,
        status: MkProjectStatus.active,
        payload: MkProjectData,
      ),
      (
        type: MkEntityType.task,
        status: MkTaskStatus.inProgress,
        payload: MkTaskData,
      ),
      (
        type: MkEntityType.person,
        status: MkPersonStatus.researching,
        payload: MkPersonData,
      ),
      (
        type: MkEntityType.interview,
        status: MkInterviewStatus.planning,
        payload: MkInterviewData,
      ),
      (
        type: MkEntityType.content,
        status: MkContentStatus.inReview,
        payload: MkContentData,
      ),
      (
        type: MkEntityType.meeting,
        status: MkMeetingStatus.planned,
        payload: MkMeetingData,
      ),
      (
        type: MkEntityType.decision,
        status: MkDecisionStatus.proposed,
        payload: MkDecisionData,
      ),
      (
        type: MkEntityType.knowledge,
        status: MkKnowledgeStatus.verified,
        payload: MkKnowledgeData,
      ),
      (
        type: MkEntityType.approval,
        status: MkApprovalStatus.pending,
        payload: MkApprovalData,
      ),
    ];

    for (var index = 0; index < cases.length; index++) {
      final testCase = cases[index];
      test('parses ${testCase.type.wireName} into typed state', () {
        final record = MkRecord.fromEvent(
          _stateEvent(
            type: testCase.type,
            status: testCase.status,
            entityId: _uuid(index + 1),
          ),
        );

        expect(record.type, testCase.type);
        expect(record.payload.runtimeType, testCase.payload);
        expect(record.payload.statusValue, testCase.status);
        expect(record.schemaVersion, mkSchemaVersion2);
      });
    }

    test('requires the v2 tags and provenance used by the relay contract', () {
      final event = _stateEvent(
        type: MkEntityType.task,
        status: MkTaskStatus.toDo,
        entityId: _uuid(30),
        tags: const [],
      );

      expect(() => MkRecord.fromEvent(event), throwsFormatException);
    });

    test('requires prev on v2 updates', () {
      final event = _stateEvent(
        type: MkEntityType.task,
        status: MkTaskStatus.inProgress,
        entityId: _uuid(31),
        version: 2,
      );

      expect(() => MkRecord.fromEvent(event), throwsFormatException);
    });

    test('rejects tag and payload disagreement', () {
      final event = _stateEvent(
        type: MkEntityType.task,
        status: MkTaskStatus.toDo,
        entityId: _uuid(32),
        statusTag: MkTaskStatus.done.wireName,
      );

      expect(() => MkRecord.fromEvent(event), throwsFormatException);
    });
  });

  group('schema v1 compatibility', () {
    test('normalizes a legacy guest status', () {
      final event = _stateEvent(
        type: MkEntityType.person,
        status: _WireStatus('research_ready'),
        entityId: _uuid(40),
        schemaVersion: mkSchemaVersion1,
      );

      final record = MkRecord.fromEvent(event);

      expect(record.status, MkPersonStatus.readyToContact.wireName);
      expect(record.payload, isA<MkPersonData>());
    });

    test('limits v1 reads to the original V0 entity kinds', () {
      final event = _stateEvent(
        type: MkEntityType.task,
        status: MkTaskStatus.toDo,
        entityId: _uuid(41),
        schemaVersion: mkSchemaVersion1,
      );

      expect(() => MkRecord.fromEvent(event), throwsFormatException);
    });
  });

  test('Quick Capture exposes the five approved operational record types', () {
    expect(MkCaptureType.values.map((type) => type.label), [
      'Guest',
      'Task',
      'Meeting',
      'Content idea',
      'Knowledge note',
    ]);
  });

  group('normalized snapshot', () {
    test('selects the highest version and retains immutable revisions', () {
      final entityId = _uuid(50);
      final revision2 = _stateEvent(
        type: MkEntityType.task,
        status: MkTaskStatus.done,
        entityId: entityId,
        version: 2,
        previousEventId: 'event-v1',
        eventId: 'event-v2',
        createdAt: 20,
      );
      final revision1 = _stateEvent(
        type: MkEntityType.task,
        status: MkTaskStatus.toDo,
        entityId: entityId,
        eventId: 'event-v1',
        createdAt: 10,
      );

      final snapshot = MkIdeasSnapshot.fromEvents([revision2, revision1]);

      expect(snapshot.records, hasLength(1));
      expect(snapshot.records.single.version, 2);
      expect(snapshot.revisionsFor(MkEntityType.task, entityId), hasLength(2));
    });

    test('isolates malformed events instead of failing the workspace', () {
      final valid = _stateEvent(
        type: MkEntityType.goal,
        status: MkGoalStatus.draft,
        entityId: _uuid(51),
      );
      const malformed = NostrEvent(
        id: 'bad',
        pubkey: 'author',
        createdAt: 2,
        kind: EventKind.mkTask,
        tags: [],
        content: '{}',
        sig: 'sig',
      );

      final snapshot = MkIdeasSnapshot.fromEvents([malformed, valid]);

      expect(snapshot.records.single.type, MkEntityType.goal);
    });

    test('indexes proposals and human approval actions by stable identity', () {
      final proposalId = _uuid(60);
      final proposal = NostrEvent(
        id: 'proposal-event',
        pubkey: 'agent-author',
        createdAt: 10,
        kind: EventKind.mkAgentProposal,
        tags: const [
          ['h', 'hub.mkideas.test'],
        ],
        content: jsonEncode({
          'schema_version': 2,
          'proposal_id': proposalId,
          'target_id': _uuid(61),
          'target_kind': EventKind.mkPerson,
          'agent': 'guest-researcher',
          'summary': 'Research ready',
          'provenance': ['https://example.test/source'],
        }),
        sig: 'sig',
      );
      final action = NostrEvent(
        id: 'approval-event',
        pubkey: 'human-author',
        createdAt: 11,
        kind: EventKind.mkApprovalAction,
        tags: const [
          ['h', 'hub.mkideas.test'],
        ],
        content: jsonEncode({
          'approval_id': _uuid(62),
          'proposal_id': proposalId,
          'decision': 'approved',
        }),
        sig: 'sig',
      );

      final pending = MkIdeasSnapshot.fromEvents([proposal]);
      final reviewed = pending.applying(action);

      expect(pending.pendingProposals, hasLength(1));
      expect(reviewed.pendingProposals, isEmpty);
      expect(reviewed.approvalActionsById, hasLength(1));
    });
  });
}

NostrEvent _stateEvent({
  required MkEntityType type,
  required MkStatusValue status,
  required String entityId,
  int schemaVersion = mkSchemaVersion2,
  int version = 1,
  String? previousEventId,
  String? statusTag,
  String? eventId,
  int createdAt = 1,
  List<List<String>>? tags,
}) {
  final content = <String, dynamic>{
    'schema_version': schemaVersion,
    'record_type': type.wireName,
    'entity_id': entityId,
    'version': version,
    'status': status.wireName,
    if (type == MkEntityType.person) ...{
      'name': '${type.label} $version',
      'do_not_contact': false,
    } else
      'title': '${type.label} $version',
    if (schemaVersion == mkSchemaVersion2) ...{
      'source': 'test',
      'provenance': {'fixture': 'mobile-model-test'},
    },
  };
  return NostrEvent(
    id: eventId ?? 'event-$entityId-$version',
    pubkey: 'author',
    createdAt: createdAt,
    kind: type.kind,
    tags:
        tags ??
        [
          ['d', entityId],
          ['h', 'hub.mkideas.test'],
          ['version', '$version'],
          ['status', statusTag ?? status.wireName],
          if (previousEventId != null) ['prev', previousEventId],
        ],
    content: jsonEncode(content),
    sig: 'sig',
  );
}

String _uuid(int value) =>
    '00000000-0000-4000-8000-${value.toString().padLeft(12, '0')}';

final class _WireStatus implements MkStatusValue {
  const _WireStatus(this.wireName);
  @override
  final String wireName;
  @override
  String get label => wireName;
}
