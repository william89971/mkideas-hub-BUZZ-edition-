import 'dart:convert';

import 'package:buzz/shared/mkideas/mkideas_provider.dart';
import 'package:buzz/shared/relay/relay.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('walks every mk-heads cursor without a fixed snapshot limit', () async {
    final seen = <NostrFilter>[];
    var call = 0;
    final repository = MkIdeasRepository(
      community: 'hub.mkideas.test',
      queryEvents: (_) async => const [],
      queryProjection: (filter) async {
        seen.add(filter);
        call += 1;
        return RelayQueryPage(
          events: [
            _stateEvent(
              call,
              entityId:
                  '00000000-0000-4000-8000-${call.toString().padLeft(12, '0')}',
            ),
          ],
          nextCursor: call == 1 ? 'next-page' : null,
        );
      },
    );

    final events = await repository.fetchAllHeads(pageSize: 1);

    expect(events, hasLength(2));
    expect(seen.first.extensions['mk_projection'], 'heads');
    expect(seen.first.extensions.containsKey('mk_cursor'), isFalse);
    expect(seen.last.extensions['mk_cursor'], 'next-page');
    expect(seen.every((filter) => filter.limit == 1), isTrue);
  });

  test(
    'loads only the requested entity history and forwards its cursor',
    () async {
      const coordinate = MkEntityCoordinate(
        type: MkEntityType.task,
        entityId: '00000000-0000-4000-8000-000000000001',
      );
      NostrFilter? seen;
      final repository = MkIdeasRepository(
        community: 'hub.mkideas.test',
        queryEvents: (_) async => const [],
        queryProjection: (filter) async {
          seen = filter;
          return RelayQueryPage(events: [_stateEvent(2)], nextCursor: 'older');
        },
      );

      final page = await repository.fetchHistoryPage(
        coordinate,
        cursor: 'newer',
      );

      expect(page.records.single.version, 2);
      expect(page.nextCursor, 'older');
      expect(seen?.extensions['mk_projection'], 'history');
      expect(seen?.extensions['mk_cursor'], 'newer');
      expect(seen?.tags['#d'], [coordinate.entityId]);
    },
  );

  test('fails closed when the relay repeats a head cursor', () async {
    final repository = MkIdeasRepository(
      community: 'hub.mkideas.test',
      queryEvents: (_) async => const [],
      queryProjection: (_) async =>
          const RelayQueryPage(events: [], nextCursor: 'same-page'),
    );

    await expectLater(repository.fetchAllHeads(), throwsFormatException);
  });

  test('rejects a history page outside the requested coordinate', () async {
    const coordinate = MkEntityCoordinate(
      type: MkEntityType.task,
      entityId: '00000000-0000-4000-8000-000000000001',
    );
    final repository = MkIdeasRepository(
      community: 'hub.mkideas.test',
      queryEvents: (_) async => const [],
      queryProjection: (_) async => RelayQueryPage(
        events: [
          _stateEvent(1, entityId: '00000000-0000-4000-8000-000000000002'),
        ],
      ),
    );

    await expectLater(
      repository.fetchHistoryPage(coordinate),
      throwsFormatException,
    );
  });
}

NostrEvent _stateEvent(
  int version, {
  String entityId = '00000000-0000-4000-8000-000000000001',
}) => NostrEvent(
  id: 'event-$entityId-$version',
  pubkey: 'human',
  createdAt: version,
  kind: EventKind.mkTask,
  tags: [
    ['d', entityId],
    ['h', 'hub.mkideas.test'],
    ['version', '$version'],
    ['status', version == 1 ? 'to-do' : 'in-progress'],
    if (version > 1) ['prev', 'event-$entityId-${version - 1}'],
  ],
  content: jsonEncode({
    'schema_version': 2,
    'record_type': 'task',
    'entity_id': entityId,
    'version': version,
    'status': version == 1 ? 'to-do' : 'in-progress',
    'title': 'Task $version',
    'source': 'test',
    'provenance': {'fixture': 'repository-test'},
  }),
  sig: 'sig',
);
