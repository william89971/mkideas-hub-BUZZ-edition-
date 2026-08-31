import 'package:buzz/shared/mkideas/mkideas_provider.dart';
import 'package:buzz/shared/relay/nostr_models.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('builds one exact schema-v2 atomic approval payload', () {
    const decision = MkAtomicApprovalDecision(
      actionId: '10000000-0000-4000-8000-000000000001',
      approvalId: '20000000-0000-4000-8000-000000000001',
      approvalEventId:
          'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
      targetId: '30000000-0000-4000-8000-000000000001',
      targetKind: 30804,
      targetEventId:
          'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
      targetVersion: 5,
      proposalId: '40000000-0000-4000-8000-000000000001',
      proposalEventId:
          'cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc',
      decision: 'approved',
      reason: 'Partner reviewed the timestamped excerpts.',
    );

    final content = decision.toContent();

    expect(content['schema_version'], 2);
    expect(content['approval_event_id'], decision.approvalEventId);
    expect(content['proposal_event_id'], decision.proposalEventId);
    expect(content['target_event_id'], decision.targetEventId);
    expect(content['target_version'], 5);
    expect(content.containsKey('result_event'), isFalse);
    expect(content.containsKey('approved'), isFalse);
  });

  test('reject cannot carry a state result', () {
    final rejected = MkAtomicApprovalDecision(
      actionId: 'action',
      approvalId: 'approval',
      approvalEventId: 'approval-event',
      targetId: 'target',
      targetKind: 30804,
      targetEventId: 'target-event',
      targetVersion: 1,
      proposalId: 'proposal',
      proposalEventId: 'proposal-event',
      decision: 'rejected',
      reason: 'The excerpt is not representative.',
      resultEvent: _dummyEvent,
    );

    expect(rejected.toContent, throwsArgumentError);
  });
}

const _dummyEvent = NostrEvent(
  id: 'result',
  pubkey: 'human',
  createdAt: 1,
  kind: 30804,
  tags: [],
  content: '{}',
  sig: 'sig',
);
