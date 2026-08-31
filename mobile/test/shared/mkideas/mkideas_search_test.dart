import 'dart:convert';

import 'package:buzz/shared/mkideas/mkideas_provider.dart';
import 'package:buzz/shared/mkideas/mkideas_search.dart';
import 'package:buzz/shared/relay/nostr_models.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  const entityId = '11111111-1111-4111-8111-111111111111';

  test('returns only the newest current entity revision', () {
    final first = _personEvent(
      eventId: 'first',
      entityId: entityId,
      version: 1,
      name: 'Avery Stone',
    );
    final second = _personEvent(
      eventId: 'second',
      entityId: entityId,
      version: 2,
      name: 'Avery Stone, Founder',
      previousEventId: first.id,
    );

    final results = MkIdeasSearchResult.fromEvents([second, first]);

    expect(results, hasLength(1));
    expect(results.single.record?.version, 2);
    expect(results.single.title, 'Avery Stone, Founder');
    expect(results.single.area, MkProductArea.people);
    expect(results.single.canonicalLink, contains('kind=30803'));
  });

  test('routes agent output through its stable target coordinate', () {
    final proposal = NostrEvent(
      id: 'proposal-event',
      pubkey: 'agent',
      createdAt: 4,
      kind: EventKind.mkAgentProposal,
      tags: const [
        ['h', 'hub.mkideas.test'],
      ],
      content: jsonEncode({
        'schema_version': 2,
        'proposal_id': '22222222-2222-4222-8222-222222222222',
        'target_id': entityId,
        'target_kind': EventKind.mkPerson,
        'agent': 'guest-researcher',
        'summary': 'Research ready',
        'provenance': ['https://example.test/source'],
      }),
      sig: 'sig',
    );

    final result = MkIdeasSearchResult.fromEvents([proposal]).single;

    expect(result.resultType, MkIdeasSearchResultType.agentResult);
    expect(result.entityType, MkEntityType.person);
    expect(result.entityId, entityId);
    expect(result.area, MkProductArea.people);
  });
}

NostrEvent _personEvent({
  required String eventId,
  required String entityId,
  required int version,
  required String name,
  String? previousEventId,
}) => NostrEvent(
  id: eventId,
  pubkey: 'human',
  createdAt: version,
  kind: EventKind.mkPerson,
  tags: [
    ['d', entityId],
    ['h', 'hub.mkideas.test'],
    ['version', '$version'],
    ['status', MkPersonStatus.researching.wireName],
    if (previousEventId != null) ['prev', previousEventId],
  ],
  content: jsonEncode({
    'schema_version': 2,
    'record_type': 'person',
    'entity_id': entityId,
    'version': version,
    'status': MkPersonStatus.researching.wireName,
    'name': name,
    'do_not_contact': false,
    'source': 'test',
    'provenance': {'fixture': 'mobile-search-test'},
  }),
  sig: 'sig',
);
