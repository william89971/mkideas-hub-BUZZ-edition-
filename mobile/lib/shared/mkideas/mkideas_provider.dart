import 'dart:async';
import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:uuid/uuid.dart';

import '../relay/nostr_models.dart';
import '../relay/relay_provider.dart';
import '../relay/relay_session.dart';
import '../relay/signed_event_relay.dart';
import 'mkideas_models.dart';
import 'mkideas_agent_models.dart';
import 'mkideas_approval_models.dart';
import 'mkideas_media_models.dart';
import 'mkideas_repository.dart';

export 'mkideas_agent_models.dart';
export 'mkideas_approval_models.dart';
export 'mkideas_media_models.dart';
export 'mkideas_models.dart';
export 'mkideas_repository.dart';

const _uuid = Uuid();
const _operationKinds = [
  EventKind.mkApprovalAction,
  EventKind.mkAgentProposal,
  EventKind.mkMigrationReceipt,
  EventKind.mkGeneratedSummary,
  EventKind.mkSystemActivity,
  EventKind.mkExternalCommunication,
];

String mkCommunityHost(String relayUrl) {
  final uri = Uri.parse(relayUrl);
  final host = uri.host.toLowerCase().replaceFirst(RegExp(r'\.$'), '');
  final defaultPort =
      (uri.scheme == 'wss' || uri.scheme == 'https') && uri.port == 443 ||
      (uri.scheme == 'ws' || uri.scheme == 'http') && uri.port == 80;
  return uri.hasPort && !defaultPort ? '$host:${uri.port}' : host;
}

@immutable
class MkIdeasSnapshot {
  const MkIdeasSnapshot._({
    required this.headsByCoordinate,
    required this.revisionsByCoordinate,
    required this.proposalsById,
    required this.approvalActionsById,
    required this.agentRunsById,
    required this.summariesById,
    this.syncIssue,
  });

  final Map<MkEntityCoordinate, MkRecord> headsByCoordinate;
  final Map<MkEntityCoordinate, List<MkRecord>> revisionsByCoordinate;
  final Map<String, MkProposal> proposalsById;
  final Map<String, MkApprovalAction> approvalActionsById;
  final Map<String, MkAgentRun> agentRunsById;
  final Map<String, MkGeneratedSummary> summariesById;
  final String? syncIssue;

  static const empty = MkIdeasSnapshot._(
    headsByCoordinate: {},
    revisionsByCoordinate: {},
    proposalsById: {},
    approvalActionsById: {},
    agentRunsById: {},
    summariesById: {},
  );

  factory MkIdeasSnapshot.fromEvents(Iterable<NostrEvent> events) {
    var snapshot = empty;
    for (final event in events) {
      snapshot = snapshot.applying(event);
    }
    return snapshot;
  }

  factory MkIdeasSnapshot.fromRecords({
    Iterable<MkRecord> records = const [],
    Iterable<MkProposal> proposals = const [],
    Iterable<MkApprovalAction> approvalActions = const [],
    Iterable<MkAgentRun> agentRuns = const [],
    Iterable<MkGeneratedSummary> summaries = const [],
  }) {
    final heads = <MkEntityCoordinate, MkRecord>{};
    final revisions = <MkEntityCoordinate, List<MkRecord>>{};
    for (final record in records) {
      revisions.putIfAbsent(record.coordinate, () => []).add(record);
    }
    for (final entry in revisions.entries) {
      entry.value.sort(_compareRecords);
      heads[entry.key] = entry.value.last;
    }
    final proposalsById = <String, MkProposal>{};
    for (final proposal in proposals) {
      final previous = proposalsById[proposal.proposalId];
      if (previous == null || _proposalIsNewer(proposal, previous)) {
        proposalsById[proposal.proposalId] = proposal;
      }
    }
    final actionsById = <String, MkApprovalAction>{};
    for (final action in approvalActions) {
      final previous = actionsById[action.approvalId];
      if (previous == null ||
          action.createdAt > previous.createdAt ||
          action.createdAt == previous.createdAt &&
              action.eventId.compareTo(previous.eventId) > 0) {
        actionsById[action.approvalId] = action;
      }
    }
    final runsById = <String, MkAgentRun>{};
    for (final run in agentRuns) {
      final previous = runsById[run.runId];
      if (previous == null || _agentRunIsNewer(run, previous)) {
        runsById[run.runId] = run;
      }
    }
    final summariesById = <String, MkGeneratedSummary>{};
    for (final summary in summaries) {
      final previous = summariesById[summary.summaryId];
      if (previous == null || _summaryIsNewer(summary, previous)) {
        summariesById[summary.summaryId] = summary;
      }
    }
    return MkIdeasSnapshot._(
      headsByCoordinate: Map.unmodifiable(heads),
      revisionsByCoordinate:
          Map<MkEntityCoordinate, List<MkRecord>>.unmodifiable({
            for (final entry in revisions.entries)
              entry.key: List<MkRecord>.unmodifiable(entry.value),
          }),
      proposalsById: Map.unmodifiable(proposalsById),
      approvalActionsById: Map.unmodifiable(actionsById),
      agentRunsById: Map.unmodifiable(runsById),
      summariesById: Map.unmodifiable(summariesById),
    );
  }

  List<MkRecord> get records {
    final values = headsByCoordinate.values.toList(growable: false);
    values.sort((a, b) => b.createdAt.compareTo(a.createdAt));
    return values;
  }

  List<MkProposal> get proposals {
    final values = proposalsById.values.toList(growable: false);
    values.sort((a, b) => b.createdAt.compareTo(a.createdAt));
    return values;
  }

  List<MkAgentRun> get agentRuns {
    final values = agentRunsById.values.toList(growable: false);
    values.sort((a, b) => b.createdAt.compareTo(a.createdAt));
    return values;
  }

  List<MkGeneratedSummary> get summaries {
    final values = summariesById.values.toList(growable: false);
    values.sort((a, b) => b.createdAt.compareTo(a.createdAt));
    return values;
  }

  List<MkRecord> recordsOfType(MkEntityType type) =>
      records.where((record) => record.type == type).toList(growable: false);

  List<MkRecord> revisionsFor(MkEntityType type, String entityId) =>
      revisionsByCoordinate[MkEntityCoordinate(
        type: type,
        entityId: entityId,
      )] ??
      const [];

  bool isProposalReviewed(String proposalId) {
    if (approvalActionsById.values.any(
      (action) => action.proposalId == proposalId,
    )) {
      return true;
    }
    return recordsOfType(MkEntityType.approval).any((record) {
      final payload = record.payload;
      return payload is MkApprovalData &&
          payload.proposalId == proposalId &&
          payload.approvalStatus != MkApprovalStatus.pending;
    });
  }

  List<MkProposal> get pendingProposals => proposals
      .where((proposal) => !isProposalReviewed(proposal.proposalId))
      .toList(growable: false);

  MkIdeasSnapshot applying(NostrEvent event) {
    try {
      final entityType = MkEntityType.fromKind(event.kind);
      if (entityType != null) {
        final record = MkRecord.fromEvent(event);
        final revisions = <MkEntityCoordinate, List<MkRecord>>{
          for (final entry in revisionsByCoordinate.entries)
            entry.key: [...entry.value],
        };
        final chain = revisions.putIfAbsent(record.coordinate, () => []);
        if (chain.every((candidate) => candidate.eventId != record.eventId)) {
          chain.add(record);
          chain.sort(_compareRecords);
        }
        final heads = {...headsByCoordinate};
        heads[record.coordinate] = chain.last;
        return MkIdeasSnapshot._(
          headsByCoordinate: Map.unmodifiable(heads),
          revisionsByCoordinate:
              Map<MkEntityCoordinate, List<MkRecord>>.unmodifiable({
                for (final entry in revisions.entries)
                  entry.key: List<MkRecord>.unmodifiable(entry.value),
              }),
          proposalsById: proposalsById,
          approvalActionsById: approvalActionsById,
          agentRunsById: agentRunsById,
          summariesById: summariesById,
          syncIssue: syncIssue,
        );
      }

      if (event.kind == EventKind.mkAgentProposal) {
        final proposal = MkProposal.fromEvent(event);
        final proposals = {...proposalsById};
        final previous = proposals[proposal.proposalId];
        if (previous == null || _proposalIsNewer(proposal, previous)) {
          proposals[proposal.proposalId] = proposal;
        }
        return MkIdeasSnapshot._(
          headsByCoordinate: headsByCoordinate,
          revisionsByCoordinate: revisionsByCoordinate,
          proposalsById: Map.unmodifiable(proposals),
          approvalActionsById: approvalActionsById,
          agentRunsById: agentRunsById,
          summariesById: summariesById,
          syncIssue: syncIssue,
        );
      }

      if (event.kind == EventKind.mkApprovalAction) {
        final action = MkApprovalAction.fromEvent(event);
        final actions = {...approvalActionsById};
        final previous = actions[action.approvalId];
        if (previous == null ||
            action.createdAt > previous.createdAt ||
            action.createdAt == previous.createdAt &&
                action.eventId.compareTo(previous.eventId) > 0) {
          actions[action.approvalId] = action;
        }
        return MkIdeasSnapshot._(
          headsByCoordinate: headsByCoordinate,
          revisionsByCoordinate: revisionsByCoordinate,
          proposalsById: proposalsById,
          approvalActionsById: Map.unmodifiable(actions),
          agentRunsById: agentRunsById,
          summariesById: summariesById,
          syncIssue: syncIssue,
        );
      }

      if (event.kind == EventKind.mkSystemActivity) {
        final run = MkAgentRun.fromEvent(event);
        final runs = {...agentRunsById};
        final previous = runs[run.runId];
        if (previous == null || _agentRunIsNewer(run, previous)) {
          runs[run.runId] = run;
        }
        return MkIdeasSnapshot._(
          headsByCoordinate: headsByCoordinate,
          revisionsByCoordinate: revisionsByCoordinate,
          proposalsById: proposalsById,
          approvalActionsById: approvalActionsById,
          agentRunsById: Map.unmodifiable(runs),
          summariesById: summariesById,
          syncIssue: syncIssue,
        );
      }

      if (event.kind == EventKind.mkGeneratedSummary) {
        final summary = MkGeneratedSummary.fromEvent(event);
        final summaries = {...summariesById};
        final previous = summaries[summary.summaryId];
        if (previous == null || _summaryIsNewer(summary, previous)) {
          summaries[summary.summaryId] = summary;
        }
        return MkIdeasSnapshot._(
          headsByCoordinate: headsByCoordinate,
          revisionsByCoordinate: revisionsByCoordinate,
          proposalsById: proposalsById,
          approvalActionsById: approvalActionsById,
          agentRunsById: agentRunsById,
          summariesById: Map.unmodifiable(summaries),
          syncIssue: syncIssue,
        );
      }
    } catch (_) {
      // Relay validation is authoritative. A malformed legacy/search event must
      // not make the entire workspace unreadable on a client upgrade.
    }
    return this;
  }

  MkIdeasSnapshot applyingRecords(Iterable<MkRecord> incoming) {
    final heads = Map<MkEntityCoordinate, MkRecord>.from(headsByCoordinate);
    final revisions = <MkEntityCoordinate, List<MkRecord>>{
      for (final entry in revisionsByCoordinate.entries)
        entry.key: [...entry.value],
    };
    for (final record in incoming) {
      final history = revisions.putIfAbsent(record.coordinate, () => []);
      if (!history.any((item) => item.eventId == record.eventId)) {
        history.add(record);
        history.sort(_compareRecords);
      }
      final head = heads[record.coordinate];
      if (head == null || _compareRecords(head, record) < 0) {
        heads[record.coordinate] = record;
      }
    }
    return MkIdeasSnapshot._(
      headsByCoordinate: Map.unmodifiable(heads),
      revisionsByCoordinate:
          Map<MkEntityCoordinate, List<MkRecord>>.unmodifiable({
            for (final entry in revisions.entries)
              entry.key: List<MkRecord>.unmodifiable(entry.value),
          }),
      proposalsById: proposalsById,
      approvalActionsById: approvalActionsById,
      agentRunsById: agentRunsById,
      summariesById: summariesById,
      syncIssue: syncIssue,
    );
  }

  MkIdeasSnapshot withSyncIssue(String? value) => MkIdeasSnapshot._(
    headsByCoordinate: headsByCoordinate,
    revisionsByCoordinate: revisionsByCoordinate,
    proposalsById: proposalsById,
    approvalActionsById: approvalActionsById,
    agentRunsById: agentRunsById,
    summariesById: summariesById,
    syncIssue: value,
  );
}

int _compareRecords(MkRecord a, MkRecord b) {
  final version = a.version.compareTo(b.version);
  if (version != 0) return version;
  final createdAt = a.createdAt.compareTo(b.createdAt);
  if (createdAt != 0) return createdAt;
  return a.eventId.compareTo(b.eventId);
}

bool _proposalIsNewer(MkProposal next, MkProposal previous) {
  final version = next.proposalVersion.compareTo(previous.proposalVersion);
  if (version != 0) return version > 0;
  final createdAt = next.createdAt.compareTo(previous.createdAt);
  if (createdAt != 0) return createdAt > 0;
  return next.eventId.compareTo(previous.eventId) > 0;
}

bool _agentRunIsNewer(MkAgentRun next, MkAgentRun previous) {
  final attempt = next.attempt.compareTo(previous.attempt);
  if (attempt != 0) return attempt > 0;
  final createdAt = next.createdAt.compareTo(previous.createdAt);
  if (createdAt != 0) return createdAt > 0;
  return next.eventId.compareTo(previous.eventId) > 0;
}

bool _summaryIsNewer(MkGeneratedSummary next, MkGeneratedSummary previous) {
  final version = next.version.compareTo(previous.version);
  if (version != 0) return version > 0;
  final createdAt = next.createdAt.compareTo(previous.createdAt);
  if (createdAt != 0) return createdAt > 0;
  return next.eventId.compareTo(previous.eventId) > 0;
}

class MkIdeasNotifier extends AsyncNotifier<MkIdeasSnapshot> {
  void Function()? _unsubscribe;

  @override
  Future<MkIdeasSnapshot> build() async {
    final config = ref.watch(relayConfigProvider);
    ref.onDispose(() => _unsubscribe?.call());
    final snapshotStartedAt = DateTime.now().millisecondsSinceEpoch ~/ 1000 - 1;
    final snapshot = await _fetch(config);
    unawaited(
      Future<void>.delayed(
        Duration.zero,
      ).then((_) => _subscribe(config, since: snapshotStartedAt)),
    );
    return snapshot;
  }

  Future<MkIdeasSnapshot> _fetch(RelayConfig config) async {
    final session = ref.read(relaySessionProvider.notifier);
    final repository = MkIdeasRepository.forRelay(
      community: mkCommunityHost(config.wsUrl),
      session: session,
    );
    final results = await Future.wait([
      repository.fetchAllHeads(),
      repository.fetchAllOperations(),
    ]);
    return MkIdeasSnapshot.fromEvents([...results[0], ...results[1]]);
  }

  /// Loads one cursor-bearing history page and merges it into normalized state.
  Future<MkEntityHistoryPage> loadHistoryPage(
    MkEntityCoordinate coordinate, {
    String? cursor,
  }) async {
    final config = ref.read(relayConfigProvider);
    final repository = MkIdeasRepository.forRelay(
      community: mkCommunityHost(config.wsUrl),
      session: ref.read(relaySessionProvider.notifier),
    );
    final page = await repository.fetchHistoryPage(coordinate, cursor: cursor);
    final current = state.value ?? MkIdeasSnapshot.empty;
    state = AsyncData(current.applyingRecords(page.records));
    return page;
  }

  Future<void> _subscribe(RelayConfig config, {required int since}) async {
    _unsubscribe?.call();
    try {
      _unsubscribe = await ref
          .read(relaySessionProvider.notifier)
          .subscribe(
            NostrFilter(
              kinds: [
                ...MkEntityType.values.map((type) => type.kind),
                ..._operationKinds,
              ],
              tags: {
                '#h': [mkCommunityHost(config.wsUrl)],
              },
              since: since,
              limit: 0,
            ),
            _applyLiveEvent,
            onClosed: (message) => _setSyncIssue(message),
          );
      _setSyncIssue(null);
    } catch (error) {
      _setSyncIssue('Live updates unavailable: $error');
    }
  }

  void _applyLiveEvent(NostrEvent event) {
    final current = state.value;
    if (current == null) return;
    state = AsyncData(current.applying(event).withSyncIssue(null));
  }

  void _setSyncIssue(String? issue) {
    final current = state.value;
    if (current == null) return;
    state = AsyncData(current.withSyncIssue(issue));
  }

  Future<NostrEvent> _publishState({
    required MkEntityType type,
    required MkStatusValue status,
    required Map<String, dynamic> fields,
    String? entityId,
    MkRecord? previous,
    int schemaVersion = mkSchemaVersion2,
  }) async {
    if (!type.statuses.any(
      (candidate) => candidate.wireName == status.wireName,
    )) {
      throw ArgumentError.value(
        status.wireName,
        'status',
        'not valid for ${type.wireName}',
      );
    }
    final config = ref.read(relayConfigProvider);
    final id = entityId ?? previous?.entityId ?? _uuid.v4();
    final version = (previous?.version ?? 0) + 1;
    final tags = <List<String>>[
      ['d', id],
      ['h', mkCommunityHost(config.wsUrl)],
      ['version', '$version'],
      ['status', status.wireName],
      if (previous != null) ['prev', previous.eventId],
    ];
    _addRelationshipTags(tags, type, fields);

    final relay = SignedEventRelay(
      session: ref.read(relaySessionProvider.notifier),
      nsec: config.nsec,
    );
    NostrEvent? signed;
    final acknowledgement = await relay.submit(
      kind: type.kind,
      tags: tags,
      content: jsonEncode({
        ...fields,
        'schema_version': schemaVersion,
        'record_type': type.wireName,
        'entity_id': id,
        'version': version,
        'status': status.wireName,
        'source': 'buzz-mobile',
        'provenance': const {'authorship': 'human', 'client': 'buzz-mobile'},
      }),
      onSigned: (event) => signed = event,
    );
    final published = signed ?? acknowledgement;
    _applyLiveEvent(published);
    return published;
  }

  Future<void> createEntity({
    required MkEntityType type,
    required MkStatusValue status,
    required String title,
    String description = '',
    Map<String, dynamic> fields = const {},
  }) async {
    final trimmedTitle = title.trim();
    if (trimmedTitle.isEmpty) {
      throw ArgumentError.value(title, 'title', 'must not be empty');
    }
    await _publishState(
      type: type,
      status: status,
      fields: {
        ...fields,
        'title': trimmedTitle,
        if (description.trim().isNotEmpty) 'description': description.trim(),
      },
    );
  }

  Future<void> capture(MkCaptureDraft draft) async {
    final title = draft.title.trim();
    if (title.isEmpty) {
      throw ArgumentError.value(draft.title, 'title', 'must not be empty');
    }
    switch (draft.type) {
      case MkCaptureType.guest:
        await _publishState(
          type: MkEntityType.person,
          status: MkPersonStatus.prospect,
          fields: {
            ...draft.contextFields,
            'name': title,
            'organization': draft.organization.trim(),
            'why_now': draft.whyNow.trim(),
            'do_not_contact': false,
          },
        );
      case MkCaptureType.task:
        await createEntity(
          type: MkEntityType.task,
          status: MkTaskStatus.toDo,
          title: title,
          description: draft.details,
          fields: draft.contextFields,
        );
      case MkCaptureType.meeting:
        await createEntity(
          type: MkEntityType.meeting,
          status: MkMeetingStatus.planned,
          title: title,
          description: draft.details,
          fields: draft.contextFields,
        );
      case MkCaptureType.contentIdea:
        await createEntity(
          type: MkEntityType.content,
          status: MkContentStatus.idea,
          title: title,
          description: draft.details,
          fields: draft.contextFields,
        );
      case MkCaptureType.knowledgeNote:
        await createEntity(
          type: MkEntityType.knowledge,
          status: MkKnowledgeStatus.draft,
          title: title,
          description: draft.details,
          fields: {...draft.contextFields, 'body': draft.details.trim()},
        );
    }
  }

  Future<void> updateStatus(MkRecord record, MkStatusValue status) async {
    await _publishState(
      type: record.type,
      status: status,
      previous: record,
      fields: {...record.data},
    );
  }

  Future<void> addGuest({
    required String name,
    required String organization,
    required String whyNow,
  }) => capture(
    MkCaptureDraft(
      type: MkCaptureType.guest,
      title: name,
      organization: organization,
      whyNow: whyNow,
    ),
  );

  Future<void> createInterview(MkRecord guest) async {
    await _publishState(
      type: MkEntityType.interview,
      status: MkInterviewStatus.planning,
      fields: {
        'title': 'Interview with ${guest.title}',
        'guest_id': guest.entityId,
      },
    );
  }

  Future<void> attachTranscript(
    MkRecord interview,
    String name,
    String text,
  ) async {
    if (interview.schemaVersion != mkSchemaVersion1) {
      throw UnsupportedError(
        'Schema-v2 transcripts require a private media upload descriptor.',
      );
    }
    await _publishState(
      type: MkEntityType.interview,
      status: MkInterviewStatus.reviewing,
      previous: interview,
      schemaVersion: mkSchemaVersion1,
      fields: {
        ...interview.data,
        'transcript_name': name,
        'transcript_text': text,
      },
    );
  }

  /// Attaches immutable private-media metadata without placing transcript text
  /// in the signed state event.
  Future<void> attachTranscriptDescriptor(
    MkRecord interview,
    MkMediaDescriptor descriptor,
  ) async {
    if (interview.type != MkEntityType.interview) {
      throw ArgumentError('Transcript target must be an interview.');
    }
    await _publishState(
      type: MkEntityType.interview,
      status: MkInterviewStatus.reviewing,
      previous: interview,
      fields: {...interview.data, 'transcript_media': descriptor.toJson()},
    );
  }

  Future<void> createContent(MkRecord interview) async {
    await _publishState(
      type: MkEntityType.content,
      status: MkContentStatus.inReview,
      fields: {
        'title': '${interview.title} — clips',
        'interview_id': interview.entityId,
      },
    );
  }

  /// Publishes one schema-v2 action that the relay applies atomically with an
  /// optional, separately human-signed resulting state event.
  Future<void> reviewProposal(
    MkProposal proposal,
    String decision, {
    String reason = 'Reviewed by an MK Ideas partner.',
    NostrEvent? resultEvent,
  }) async {
    if (decision != 'approved' && decision != 'rejected') {
      throw ArgumentError.value(
        decision,
        'decision',
        'must be approved or rejected',
      );
    }
    if (!proposal.isApprovable) {
      throw StateError(
        proposal.isStale
            ? 'This proposal is stale. Run the agent again against the current record.'
            : 'This proposal predates the atomic approval contract.',
      );
    }
    if (decision == 'rejected' && resultEvent != null) {
      throw ArgumentError(
        'A rejected proposal cannot carry a resulting event.',
      );
    }
    final trimmedReason = reason.trim();
    if (trimmedReason.isEmpty) {
      throw ArgumentError.value(reason, 'reason', 'must not be empty');
    }
    final snapshot = state.value ?? MkIdeasSnapshot.empty;
    final targetType = MkEntityType.fromKind(proposal.targetKind);
    if (targetType == null || targetType == MkEntityType.approval) {
      throw StateError('Proposal target is not protected MK Ideas state.');
    }
    final target =
        snapshot.headsByCoordinate[MkEntityCoordinate(
          type: targetType,
          entityId: proposal.targetId,
        )];
    if (target == null ||
        target.eventId != proposal.targetEventId ||
        target.version != proposal.targetVersion) {
      throw StateError(
        'The proposal target changed. Refresh and run the agent again.',
      );
    }
    MkRecord? approval;
    for (final candidate in snapshot.recordsOfType(MkEntityType.approval)) {
      final payload = candidate.payload;
      if (payload is MkApprovalData &&
          payload.approvalStatus == MkApprovalStatus.pending &&
          payload.proposalId == proposal.proposalId &&
          payload.proposalEventId == proposal.eventId &&
          payload.targetId == proposal.targetId &&
          payload.targetKind == proposal.targetKind &&
          payload.targetEventId == proposal.targetEventId &&
          payload.targetVersion == proposal.targetVersion) {
        approval = candidate;
        break;
      }
    }
    if (approval == null || approval.schemaVersion != mkSchemaVersion2) {
      throw StateError('No matching schema-v2 approval request is pending.');
    }
    final config = ref.read(relayConfigProvider);
    final relay = SignedEventRelay(
      session: ref.read(relaySessionProvider.notifier),
      nsec: config.nsec,
    );
    NostrEvent? signed;
    await relay.submit(
      kind: EventKind.mkApprovalAction,
      tags: [
        ['h', mkCommunityHost(config.wsUrl)],
        ['e', proposal.eventId],
        ['approval', approval.entityId, approval.eventId],
        ['target', '${proposal.targetKind}', proposal.targetId],
        ['proposal', proposal.proposalId, proposal.eventId],
      ],
      content: jsonEncode(
        MkAtomicApprovalDecision(
          actionId: _uuid.v4(),
          approvalId: approval.entityId,
          approvalEventId: approval.eventId,
          targetId: proposal.targetId,
          targetKind: proposal.targetKind,
          targetEventId: proposal.targetEventId!,
          targetVersion: proposal.targetVersion!,
          proposalId: proposal.proposalId,
          proposalEventId: proposal.eventId,
          decision: decision,
          reason: trimmedReason,
          resultEvent: resultEvent,
        ).toContent(),
      ),
      onSigned: (event) => signed = event,
    );
    if (signed case final action?) {
      _applyLiveEvent(action);
      if (resultEvent != null) _applyLiveEvent(resultEvent);
    }
  }
}

void _addRelationshipTags(
  List<List<String>> tags,
  MkEntityType type,
  Map<String, dynamic> fields,
) {
  if (type == MkEntityType.interview && fields['guest_id'] is String) {
    tags.add(['guest', fields['guest_id'] as String]);
  }
  if (type == MkEntityType.content && fields['interview_id'] is String) {
    tags.add(['interview', fields['interview_id'] as String]);
  }
  if (type == MkEntityType.task && fields['project_id'] is String) {
    tags.add(['project', fields['project_id'] as String]);
  }
  if (type == MkEntityType.operationalProject && fields['goal_id'] is String) {
    tags.add(['goal', fields['goal_id'] as String]);
  }
  if (type == MkEntityType.approval &&
      fields['target_id'] is String &&
      fields['target_kind'] is int &&
      fields['proposal_id'] is String &&
      fields['proposal_event_id'] is String) {
    tags.add([
      'target',
      '${fields['target_kind']}',
      fields['target_id'] as String,
    ]);
    tags.add([
      'proposal',
      fields['proposal_id'] as String,
      fields['proposal_event_id'] as String,
    ]);
  }
}

final mkIdeasProvider = AsyncNotifierProvider<MkIdeasNotifier, MkIdeasSnapshot>(
  MkIdeasNotifier.new,
);
