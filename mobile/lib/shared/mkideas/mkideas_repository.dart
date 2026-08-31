import '../relay/relay.dart';
import 'mkideas_models.dart';

/// Executes one signed-event query through Buzz's ordinary `/query` bridge.
typedef MkEventQuery =
    Future<List<NostrEvent>> Function(List<NostrFilter> filters);

/// Executes one cursor-bearing MK projection query.
typedef MkProjectionQuery = Future<RelayQueryPage> Function(NostrFilter filter);

/// A validated page of immutable revisions for one shared MK entity.
class MkEntityHistoryPage {
  const MkEntityHistoryPage({required this.records, this.nextCursor});

  /// Accepted revisions sorted from oldest to newest.
  final List<MkRecord> records;

  /// Opaque server-issued cursor, or null when history is complete.
  final String? nextCursor;
}

/// Typed boundary around Buzz's MK head/history projection extension.
class MkIdeasRepository {
  const MkIdeasRepository({
    required this.community,
    required MkEventQuery queryEvents,
    required MkProjectionQuery queryProjection,
  }) : _queryEvents = queryEvents,
       _queryProjection = queryProjection;

  /// Builds the repository against an authenticated relay session.
  factory MkIdeasRepository.forRelay({
    required String community,
    required RelaySessionNotifier session,
  }) => MkIdeasRepository(
    community: community,
    queryEvents: session.queryRelay,
    queryProjection: session.queryRelayPage,
  );

  /// Current community host selected by the app.
  final String community;

  final MkEventQuery _queryEvents;
  final MkProjectionQuery _queryProjection;

  /// Loads every current shared head using the server's stable cursor.
  Future<List<NostrEvent>> fetchAllHeads({int pageSize = 200}) async {
    final events = <NostrEvent>[];
    final coordinates = <MkEntityCoordinate>{};
    String? cursor;
    final seenCursors = <String>{};
    do {
      final page = await _queryProjection(
        NostrFilter(
          kinds: MkEntityType.values.map((type) => type.kind).toList(),
          tags: {
            '#h': [community],
          },
          limit: pageSize,
          extensions: {'mk_projection': 'heads', 'mk_cursor': ?cursor},
        ),
      );
      for (final event in page.events) {
        if (event.getTagValue('h') != community) {
          throw const FormatException(
            'relay returned an MK head outside the requested community',
          );
        }
        final record = MkRecord.fromEvent(event);
        if (!coordinates.add(record.coordinate)) {
          throw const FormatException(
            'relay repeated an MK head coordinate across pages',
          );
        }
        events.add(event);
      }
      final next = page.nextCursor;
      if (next != null && !seenCursors.add(next)) {
        throw const FormatException('relay repeated an MK head cursor');
      }
      cursor = next;
    } while (cursor != null);
    return List.unmodifiable(events);
  }

  /// Loads one page of authorized immutable history for [coordinate].
  Future<MkEntityHistoryPage> fetchHistoryPage(
    MkEntityCoordinate coordinate, {
    String? cursor,
    int pageSize = 100,
  }) async {
    final page = await _queryProjection(
      NostrFilter(
        kinds: [coordinate.type.kind],
        tags: {
          '#h': [community],
          '#d': [coordinate.entityId],
        },
        limit: pageSize,
        extensions: {'mk_projection': 'history', 'mk_cursor': ?cursor},
      ),
    );
    final records = <MkRecord>[];
    for (final event in page.events) {
      if (event.getTagValue('h') != community) {
        throw const FormatException(
          'relay returned MK history outside the requested community',
        );
      }
      final record = MkRecord.fromEvent(event);
      if (record.coordinate != coordinate) {
        throw const FormatException(
          'relay returned history outside the requested coordinate',
        );
      }
      records.add(record);
    }
    records.sort((a, b) => a.version.compareTo(b.version));
    return MkEntityHistoryPage(
      records: List.unmodifiable(records),
      nextCursor: page.nextCursor,
    );
  }

  /// Loads recent append-only operational events used by Today and agents.
  Future<List<NostrEvent>> fetchRecentOperations({int limit = 200}) =>
      _queryEvents([
        NostrFilter(
          kinds: const [
            EventKind.mkApprovalAction,
            EventKind.mkAgentProposal,
            EventKind.mkMigrationReceipt,
            EventKind.mkGeneratedSummary,
            EventKind.mkSystemActivity,
          ],
          tags: {
            '#h': [community],
          },
          limit: limit,
        ),
      ]);
}
