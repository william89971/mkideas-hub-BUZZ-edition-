import 'dart:convert';

import 'package:buzz/shared/mkideas/mkideas_provider.dart';
import 'package:buzz/shared/relay/nostr_models.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('parses all five human-gated personas', () {
    expect(MkAgentPersona.values, hasLength(5));
    expect(MkAgentPersona.outreachDrafter.draftOnly, isTrue);
    expect(
      MkAgentPersona.operationsBriefingAssistant.informationalOnly,
      isTrue,
    );
  });

  test('parses fixture-shaped nested proposal and provenance', () {
    final proposal = MkProposal.fromEvent(
      _event(
        kind: EventKind.mkAgentProposal,
        content: {
          'schema_version': 2,
          'proposal_id': '20000000-0000-4000-8000-000000000004',
          'proposal_version': 1,
          'run_id': '10000000-0000-4000-8000-000000000004',
          'persona_id': 'content-clip-copilot',
          'target': {
            'kind': EventKind.mkInterview,
            'id': '22222222-2222-4222-8222-222222222222',
            'event_id':
                'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
            'version': 5,
          },
          'proposal_type': 'timestamped_clips',
          'summary': 'Two transcript-grounded clips.',
          'provenance': [
            {
              'source_id': 'media:transcript-v1',
              'source_type': 'private_transcript',
              'title': 'Interview transcript',
              'locator': 'media://transcript-v1',
              'sha256': 'hash',
            },
          ],
          'output': {
            'clips': [
              {
                'start_ms': 4000,
                'end_ms': 18000,
                'title': 'Human judgment',
                'caption': 'A draft caption.',
              },
            ],
          },
          'status': 'proposed',
          'review_state': 'pending',
        },
      ),
    );

    expect(proposal.personaId, 'content-clip-copilot');
    expect(proposal.targetVersion, 5);
    expect(proposal.sources.single.title, 'Interview transcript');
    expect(proposal.clips.single['start_ms'], 4000);
    expect(proposal.isApprovable, isTrue);
  });

  test('stale proposal is retained but cannot be approved', () {
    final proposal = MkProposal.fromEvent(
      _event(
        kind: EventKind.mkAgentProposal,
        content: {
          'schema_version': 2,
          'proposal_id': '40000000-0000-4000-8000-000000000001',
          'proposal_version': 1,
          'persona_id': 'guest-researcher',
          'target': {
            'kind': EventKind.mkPerson,
            'id': '11111111-1111-4111-8111-111111111111',
            'event_id':
                'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
            'version': 7,
          },
          'summary': 'Historical research draft.',
          'review_state': 'stale',
          'output': const {},
        },
      ),
    );

    expect(proposal.isStale, isTrue);
    expect(proposal.isApprovable, isFalse);
  });

  test('parses durable failed run state without blocking manual work', () {
    final run = MkAgentRun.fromEvent(
      _event(
        kind: EventKind.mkSystemActivity,
        content: {
          'schema_version': 2,
          'activity_type': 'agent_run',
          'run_id': '30000000-0000-4000-8000-000000000002',
          'persona_id': 'outreach-drafter',
          'status': 'failed',
          'attempt': 1,
          'error': {
            'code': 'dnc_active',
            'message': 'Drafting stopped because DNC is enabled.',
            'retryable': false,
          },
        },
      ),
    );

    expect(run.status, MkAgentRunStatus.failed);
    expect(run.errorCode, 'dnc_active');
    expect(run.retryable, isFalse);
  });
}

NostrEvent _event({required int kind, required Map<String, dynamic> content}) =>
    NostrEvent(
      id: 'event-$kind',
      pubkey: 'service',
      createdAt: 10,
      kind: kind,
      tags: const [
        ['h', 'hub.mkideas.test'],
      ],
      content: jsonEncode(content),
      sig: 'sig',
    );
