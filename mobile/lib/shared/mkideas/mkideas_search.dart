import '../deeplink/deep_link.dart';
import '../relay/nostr_models.dart';
import 'mkideas_models.dart';

/// The typed categories rendered by universal MK Ideas search.
enum MkIdeasSearchResultType { entity, agentResult }

/// A search hit resolved to a stable MK Ideas entity coordinate.
class MkIdeasSearchResult {
  const MkIdeasSearchResult({
    required this.resultType,
    required this.entityType,
    required this.entityId,
    required this.community,
    required this.title,
    required this.summary,
    required this.createdAt,
    required this.sourceEventId,
    this.record,
    this.proposal,
  });

  final MkIdeasSearchResultType resultType;
  final MkEntityType entityType;
  final String entityId;
  final String community;
  final String title;
  final String summary;
  final int createdAt;
  final String sourceEventId;
  final MkRecord? record;
  final MkProposal? proposal;

  MkProductArea get area => entityType.productArea;

  String get canonicalLink => buildMkIdeasEntityLink(
    community: community,
    kind: entityType.kind,
    entityId: entityId,
  );

  String get stableKey => '${entityType.kind}:$entityId';

  /// Parses, validates, de-duplicates, and sorts signed search events.
  static List<MkIdeasSearchResult> fromEvents(Iterable<NostrEvent> events) {
    final records = <MkEntityCoordinate, MkRecord>{};
    final recordCommunities = <MkEntityCoordinate, String>{};
    final proposals = <String, MkIdeasSearchResult>{};

    for (final event in events) {
      final entityType = MkEntityType.fromKind(event.kind);
      if (entityType != null) {
        try {
          final record = MkRecord.fromEvent(event);
          final coordinate = record.coordinate;
          final previous = records[coordinate];
          if (previous == null || _recordIsNewer(record, previous)) {
            records[coordinate] = record;
            recordCommunities[coordinate] = event.getTagValue('h')!;
          }
        } catch (_) {
          // Search remains useful when a stale or malformed indexed row exists.
        }
        continue;
      }

      if (event.kind == EventKind.mkAgentProposal) {
        try {
          final proposal = MkProposal.fromEvent(event);
          final targetType = MkEntityType.fromKind(proposal.targetKind);
          final community = event.getTagValue('h');
          if (targetType == null || community == null) continue;
          buildMkIdeasEntityLink(
            community: community,
            kind: targetType.kind,
            entityId: proposal.targetId,
          );
          final result = MkIdeasSearchResult(
            resultType: MkIdeasSearchResultType.agentResult,
            entityType: targetType,
            entityId: proposal.targetId,
            community: community,
            title: proposal.summary,
            summary:
                '${proposal.agent} · ${proposal.provenance.length} sources',
            createdAt: proposal.createdAt,
            sourceEventId: proposal.eventId,
            proposal: proposal,
          );
          final previous = proposals[proposal.proposalId];
          if (previous == null || _resultIsNewer(result, previous)) {
            proposals[proposal.proposalId] = result;
          }
        } catch (_) {
          // Agent output without a valid target is not navigable search state.
        }
      }
    }

    final results = <MkIdeasSearchResult>[
      for (final entry in records.entries)
        MkIdeasSearchResult(
          resultType: MkIdeasSearchResultType.entity,
          entityType: entry.value.type,
          entityId: entry.value.entityId,
          community: recordCommunities[entry.key]!,
          title: entry.value.title,
          summary: _recordSummary(entry.value),
          createdAt: entry.value.createdAt,
          sourceEventId: entry.value.eventId,
          record: entry.value,
        ),
      ...proposals.values,
    ];
    results.sort((a, b) {
      final created = b.createdAt.compareTo(a.createdAt);
      return created != 0 ? created : a.stableKey.compareTo(b.stableKey);
    });
    return List.unmodifiable(results);
  }
}

bool _recordIsNewer(MkRecord next, MkRecord previous) {
  final version = next.version.compareTo(previous.version);
  if (version != 0) return version > 0;
  final createdAt = next.createdAt.compareTo(previous.createdAt);
  if (createdAt != 0) return createdAt > 0;
  return next.eventId.compareTo(previous.eventId) > 0;
}

bool _resultIsNewer(MkIdeasSearchResult next, MkIdeasSearchResult previous) {
  final nextVersion = next.proposal?.proposalVersion ?? 1;
  final previousVersion = previous.proposal?.proposalVersion ?? 1;
  if (nextVersion != previousVersion) return nextVersion > previousVersion;
  if (next.createdAt != previous.createdAt) {
    return next.createdAt > previous.createdAt;
  }
  return next.sourceEventId.compareTo(previous.sourceEventId) > 0;
}

String _recordSummary(MkRecord record) {
  for (final key in ['description', 'organization', 'why_now', 'body']) {
    final value = record.data[key];
    if (value is String && value.trim().isNotEmpty) {
      final normalized = value.trim().replaceAll(RegExp(r'\s+'), ' ');
      return normalized.length <= 160
          ? normalized
          : '${normalized.substring(0, 157)}…';
    }
  }
  return record.status.replaceAll('-', ' ');
}
