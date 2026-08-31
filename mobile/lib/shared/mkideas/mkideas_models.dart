import 'dart:convert';

import 'package:flutter/foundation.dart';

import '../relay/nostr_models.dart';
import 'mkideas_media_models.dart';

const mkSchemaVersion1 = 1;
const mkSchemaVersion2 = 2;

/// The ten addressable record types reserved by NIP-MK.
enum MkEntityType {
  goal(EventKind.mkGoal, 'goal', 'Goal'),
  operationalProject(
    EventKind.mkOperationalProject,
    'operational_project',
    'Project',
  ),
  task(EventKind.mkTask, 'task', 'Task'),
  person(EventKind.mkPerson, 'person', 'Person'),
  interview(EventKind.mkInterview, 'interview', 'Interview'),
  content(EventKind.mkContent, 'content', 'Content'),
  meeting(EventKind.mkMeeting, 'meeting', 'Meeting'),
  decision(EventKind.mkDecision, 'decision', 'Decision'),
  knowledge(EventKind.mkKnowledge, 'knowledge', 'Knowledge'),
  approval(EventKind.mkApproval, 'approval', 'Approval');

  const MkEntityType(this.kind, this.wireName, this.label);

  final int kind;
  final String wireName;
  final String label;

  static MkEntityType? fromKind(int kind) {
    for (final type in values) {
      if (type.kind == kind) return type;
    }
    return null;
  }

  static MkEntityType? fromWireName(String value) {
    for (final type in values) {
      if (type.wireName == value) return type;
    }
    return null;
  }

  List<MkStatusValue> get statuses => switch (this) {
    MkEntityType.goal => [...MkGoalStatus.values],
    MkEntityType.operationalProject => [...MkProjectStatus.values],
    MkEntityType.task => [...MkTaskStatus.values],
    MkEntityType.person => [...MkPersonStatus.values],
    MkEntityType.interview => [...MkInterviewStatus.values],
    MkEntityType.content => [...MkContentStatus.values],
    MkEntityType.meeting => [...MkMeetingStatus.values],
    MkEntityType.decision => [...MkDecisionStatus.values],
    MkEntityType.knowledge => [...MkKnowledgeStatus.values],
    MkEntityType.approval => [...MkApprovalStatus.values],
  };

  MkStatusValue get initialStatus => switch (this) {
    MkEntityType.goal => MkGoalStatus.draft,
    MkEntityType.operationalProject => MkProjectStatus.planned,
    MkEntityType.task => MkTaskStatus.toDo,
    MkEntityType.person => MkPersonStatus.prospect,
    MkEntityType.interview => MkInterviewStatus.planning,
    MkEntityType.content => MkContentStatus.idea,
    MkEntityType.meeting => MkMeetingStatus.planned,
    MkEntityType.decision => MkDecisionStatus.proposed,
    MkEntityType.knowledge => MkKnowledgeStatus.draft,
    MkEntityType.approval => MkApprovalStatus.pending,
  };
}

/// The permanent MK Ideas area responsible for an operational record.
enum MkProductArea { today, work, people, studio }

extension MkEntityTypeArea on MkEntityType {
  /// Resolves an entity kind to its permanent product area.
  MkProductArea get productArea => switch (this) {
    MkEntityType.approval => MkProductArea.today,
    MkEntityType.person => MkProductArea.people,
    MkEntityType.interview || MkEntityType.content => MkProductArea.studio,
    MkEntityType.goal ||
    MkEntityType.operationalProject ||
    MkEntityType.task ||
    MkEntityType.meeting ||
    MkEntityType.decision ||
    MkEntityType.knowledge => MkProductArea.work,
  };
}

abstract interface class MkStatusValue {
  String get wireName;
  String get label;
}

enum MkGoalStatus implements MkStatusValue {
  draft('draft', 'Draft'),
  active('active', 'Active'),
  onHold('on-hold', 'On hold'),
  completed('completed', 'Completed'),
  archived('archived', 'Archived');

  const MkGoalStatus(this.wireName, this.label);
  @override
  final String wireName;
  @override
  final String label;
}

enum MkProjectStatus implements MkStatusValue {
  planned('planned', 'Planned'),
  active('active', 'Active'),
  blocked('blocked', 'Blocked'),
  completed('completed', 'Completed'),
  archived('archived', 'Archived');

  const MkProjectStatus(this.wireName, this.label);
  @override
  final String wireName;
  @override
  final String label;
}

enum MkTaskStatus implements MkStatusValue {
  backlog('backlog', 'Backlog'),
  toDo('to-do', 'To do'),
  inProgress('in-progress', 'In progress'),
  blocked('blocked', 'Blocked'),
  review('review', 'Review'),
  done('done', 'Done'),
  cancelled('cancelled', 'Cancelled');

  const MkTaskStatus(this.wireName, this.label);
  @override
  final String wireName;
  @override
  final String label;
}

enum MkPersonStatus implements MkStatusValue {
  prospect('prospect', 'Prospect'),
  researching('researching', 'Researching'),
  readyToContact('ready-to-contact', 'Ready to contact'),
  contacted('contacted', 'Contacted'),
  responded('responded', 'Responded'),
  scheduled('scheduled', 'Scheduled'),
  interviewed('interviewed', 'Interviewed'),
  nurture('nurture', 'Nurture'),
  closed('closed', 'Closed'),
  archived('archived', 'Archived');

  const MkPersonStatus(this.wireName, this.label);
  @override
  final String wireName;
  @override
  final String label;
}

enum MkInterviewStatus implements MkStatusValue {
  idea('idea', 'Idea'),
  planning('planning', 'Planning'),
  scheduled('scheduled', 'Scheduled'),
  recorded('recorded', 'Recorded'),
  transcribing('transcribing', 'Transcribing'),
  reviewing('reviewing', 'Reviewing'),
  complete('complete', 'Complete'),
  cancelled('cancelled', 'Cancelled');

  const MkInterviewStatus(this.wireName, this.label);
  @override
  final String wireName;
  @override
  final String label;
}

enum MkContentStatus implements MkStatusValue {
  idea('idea', 'Idea'),
  draft('draft', 'Draft'),
  inReview('in-review', 'In review'),
  approved('approved', 'Approved'),
  scheduled('scheduled', 'Scheduled'),
  published('published', 'Published'),
  archived('archived', 'Archived');

  const MkContentStatus(this.wireName, this.label);
  @override
  final String wireName;
  @override
  final String label;
}

enum MkMeetingStatus implements MkStatusValue {
  planned('planned', 'Planned'),
  completed('completed', 'Completed'),
  cancelled('cancelled', 'Cancelled');

  const MkMeetingStatus(this.wireName, this.label);
  @override
  final String wireName;
  @override
  final String label;
}

enum MkDecisionStatus implements MkStatusValue {
  proposed('proposed', 'Proposed'),
  decided('decided', 'Decided'),
  superseded('superseded', 'Superseded'),
  archived('archived', 'Archived');

  const MkDecisionStatus(this.wireName, this.label);
  @override
  final String wireName;
  @override
  final String label;
}

enum MkKnowledgeStatus implements MkStatusValue {
  draft('draft', 'Draft'),
  verified('verified', 'Verified'),
  archived('archived', 'Archived');

  const MkKnowledgeStatus(this.wireName, this.label);
  @override
  final String wireName;
  @override
  final String label;
}

enum MkApprovalStatus implements MkStatusValue {
  pending('pending', 'Pending'),
  approved('approved', 'Approved'),
  rejected('rejected', 'Rejected'),
  stale('stale', 'Stale'),
  cancelled('cancelled', 'Cancelled');

  const MkApprovalStatus(this.wireName, this.label);
  @override
  final String wireName;
  @override
  final String label;
}

@immutable
class MkEntityCoordinate {
  const MkEntityCoordinate({required this.type, required this.entityId});

  final MkEntityType type;
  final String entityId;

  @override
  bool operator ==(Object other) =>
      other is MkEntityCoordinate &&
      other.type == type &&
      other.entityId == entityId;

  @override
  int get hashCode => Object.hash(type, entityId);
}

sealed class MkEntityData {
  const MkEntityData(this.raw);

  final Map<String, dynamic> raw;
  MkStatusValue get statusValue;
  String get title;

  String? get description => _optionalString(raw, [
    'description',
    'details',
    'body',
    'agenda',
    'why_now',
    'outcome',
  ]);
}

final class MkGoalData extends MkEntityData {
  const MkGoalData(super.raw, this.goalStatus);
  final MkGoalStatus goalStatus;
  @override
  MkStatusValue get statusValue => goalStatus;
  @override
  String get title => _recordTitle(raw, fallback: 'Untitled goal');
}

final class MkProjectData extends MkEntityData {
  const MkProjectData(super.raw, this.projectStatus);
  final MkProjectStatus projectStatus;
  @override
  MkStatusValue get statusValue => projectStatus;
  @override
  String get title => _recordTitle(raw, fallback: 'Untitled project');
}

final class MkTaskData extends MkEntityData {
  const MkTaskData(super.raw, this.taskStatus);
  final MkTaskStatus taskStatus;
  @override
  MkStatusValue get statusValue => taskStatus;
  @override
  String get title => _recordTitle(raw, fallback: 'Untitled task');
}

final class MkPersonData extends MkEntityData {
  const MkPersonData(super.raw, this.personStatus);
  final MkPersonStatus personStatus;
  @override
  MkStatusValue get statusValue => personStatus;
  @override
  String get title => _recordTitle(raw, fallback: 'Unnamed person');
  bool get doNotContact => raw['do_not_contact'] == true;
  String? get organization => _optionalString(raw, ['organization']);
  String? get titleOrRole => _optionalString(raw, ['title', 'role']);
  String? get email => _optionalString(raw, ['email']);
  String? get owner => _optionalString(raw, ['owner', 'owner_pubkey']);
  String? get whyNow => _optionalString(raw, ['why_now']);
  List<String> get topics => _stringList(raw['topics']);
  List<String> get tags => _stringList(raw['tags']);
}

final class MkInterviewData extends MkEntityData {
  const MkInterviewData(super.raw, this.interviewStatus);
  final MkInterviewStatus interviewStatus;
  @override
  MkStatusValue get statusValue => interviewStatus;
  @override
  String get title => _recordTitle(raw, fallback: 'Untitled interview');
  String? get guestId => _optionalString(raw, ['guest_id']);
  String? get scheduledAt => _optionalString(raw, ['scheduled_at']);
  List<String> get questions => _stringList(raw['questions']);
  MkMediaDescriptor? get transcript =>
      MkMediaDescriptor.tryParse(raw['transcript_media'] ?? raw['transcript']);
}

final class MkContentData extends MkEntityData {
  const MkContentData(super.raw, this.contentStatus);
  final MkContentStatus contentStatus;
  @override
  MkStatusValue get statusValue => contentStatus;
  @override
  String get title => _recordTitle(raw, fallback: 'Untitled content');
  String? get interviewId => _optionalString(raw, ['interview_id']);
  String? get platform => _optionalString(raw, ['platform']);
  String? get publicationUrl => _optionalString(raw, ['publication_url']);
}

final class MkMeetingData extends MkEntityData {
  const MkMeetingData(super.raw, this.meetingStatus);
  final MkMeetingStatus meetingStatus;
  @override
  MkStatusValue get statusValue => meetingStatus;
  @override
  String get title => _recordTitle(raw, fallback: 'Untitled meeting');
}

final class MkDecisionData extends MkEntityData {
  const MkDecisionData(super.raw, this.decisionStatus);
  final MkDecisionStatus decisionStatus;
  @override
  MkStatusValue get statusValue => decisionStatus;
  @override
  String get title => _recordTitle(raw, fallback: 'Untitled decision');
}

final class MkKnowledgeData extends MkEntityData {
  const MkKnowledgeData(super.raw, this.knowledgeStatus);
  final MkKnowledgeStatus knowledgeStatus;
  @override
  MkStatusValue get statusValue => knowledgeStatus;
  @override
  String get title => _recordTitle(raw, fallback: 'Untitled note');
}

final class MkApprovalData extends MkEntityData {
  const MkApprovalData(super.raw, this.approvalStatus);
  final MkApprovalStatus approvalStatus;
  @override
  MkStatusValue get statusValue => approvalStatus;
  @override
  String get title => _recordTitle(raw, fallback: 'Approval request');
  String? get proposalId => _optionalString(raw, ['proposal_id']);
  String? get proposalEventId => _optionalString(raw, ['proposal_event_id']);
  String? get targetId => _optionalString(raw, ['target_id']);
  int? get targetKind => _optionalInt(raw, 'target_kind');
  String? get targetEventId => _optionalString(raw, ['target_event_id']);
  int? get targetVersion => _optionalInt(raw, 'target_version');
}

@immutable
class MkRecord {
  const MkRecord({
    required this.eventId,
    required this.author,
    required this.kind,
    required this.createdAt,
    required this.entityId,
    required this.version,
    required this.schemaVersion,
    required this.type,
    required this.payload,
    this.previousEventId,
    this.sourceStatus,
  });

  final String eventId;
  final String author;
  final int kind;
  final int createdAt;
  final String entityId;
  final int version;
  final int schemaVersion;
  final MkEntityType type;
  final MkEntityData payload;
  final String? previousEventId;
  final String? sourceStatus;

  String get recordType => type.wireName;
  String get status => payload.statusValue.wireName;
  String get rawStatus => sourceStatus ?? status;
  String get title => payload.title;
  Map<String, dynamic> get data => payload.raw;
  MkEntityCoordinate get coordinate =>
      MkEntityCoordinate(type: type, entityId: entityId);

  factory MkRecord.fromEvent(NostrEvent event) {
    final type = MkEntityType.fromKind(event.kind);
    if (type == null) {
      throw FormatException('Kind ${event.kind} is not MK addressable state');
    }
    final content = _decodeObject(event.content);
    final schemaVersion = _requiredInt(content, 'schema_version');
    if (schemaVersion != mkSchemaVersion1 &&
        schemaVersion != mkSchemaVersion2) {
      throw FormatException('Unsupported MK schema version $schemaVersion');
    }
    final recordType = _requiredString(content, 'record_type');
    if (recordType != type.wireName) {
      throw FormatException(
        'Kind ${event.kind} requires record_type ${type.wireName}',
      );
    }
    final entityId = _requiredString(content, 'entity_id');
    final version = _requiredInt(content, 'version');
    if (version < 1) throw const FormatException('version must be positive');
    final rawStatus = _requiredString(content, 'status');

    final dTag = event.getTagValue('d');
    final communityTag = event.getTagValue('h');
    final versionTag = int.tryParse(event.getTagValue('version') ?? '');
    final statusTag = event.getTagValue('status');
    if (dTag != null && dTag != entityId) {
      throw const FormatException('d tag does not match entity_id');
    }
    if (versionTag != null && versionTag != version) {
      throw const FormatException('version tag does not match payload');
    }
    if (statusTag != null && statusTag != rawStatus) {
      throw const FormatException('status tag does not match payload');
    }
    if (schemaVersion == mkSchemaVersion1 &&
        type != MkEntityType.person &&
        type != MkEntityType.interview &&
        type != MkEntityType.content &&
        type != MkEntityType.approval) {
      throw FormatException('Schema v1 is not supported for ${type.wireName}');
    }
    if (schemaVersion == mkSchemaVersion2) {
      if (!_isUuid(entityId)) {
        throw const FormatException('entity_id must be a UUID');
      }
      if (dTag == null || versionTag == null || statusTag == null) {
        throw const FormatException(
          'Schema v2 requires d, version, and status tags',
        );
      }
      if (communityTag == null || communityTag.trim().isEmpty) {
        throw const FormatException('Schema v2 requires an h community tag');
      }
      final previous = event.getTagValue('prev');
      if (version == 1 && previous != null) {
        throw const FormatException('Creation must not include prev');
      }
      if (version > 1 && (previous == null || previous.trim().isEmpty)) {
        throw const FormatException('Updates require prev');
      }
      _requiredString(content, 'source');
      final provenance = content['provenance'];
      if (provenance is! Map<String, dynamic> || provenance.isEmpty) {
        throw const FormatException('provenance must be a non-empty object');
      }
    }

    return MkRecord(
      eventId: event.id,
      author: event.pubkey,
      kind: event.kind,
      createdAt: event.createdAt,
      entityId: entityId,
      version: version,
      schemaVersion: schemaVersion,
      type: type,
      payload: _parseEntityData(type, content, rawStatus),
      previousEventId: event.getTagValue('prev'),
      sourceStatus: rawStatus,
    );
  }
}

@immutable
class MkProposal {
  const MkProposal({
    required this.eventId,
    required this.proposalId,
    required this.targetId,
    required this.targetKind,
    required this.agent,
    required this.summary,
    required this.provenance,
    required this.clips,
    required this.createdAt,
    this.schemaVersion = mkSchemaVersion1,
    this.proposalVersion = 1,
    this.targetEventId,
    this.targetVersion,
    this.runId,
    this.provider,
    this.model,
    this.outcome = 'proposed',
    this.personaId,
    this.proposalType,
    this.reviewState = 'pending',
    this.output = const {},
    this.sources = const [],
  });

  final String eventId;
  final String proposalId;
  final String targetId;
  final int targetKind;
  final String agent;
  final String summary;
  final List<String> provenance;
  final List<Map<String, dynamic>> clips;
  final int createdAt;
  final int schemaVersion;
  final int proposalVersion;
  final String? targetEventId;
  final int? targetVersion;
  final String? runId;
  final String? provider;
  final String? model;
  final String outcome;
  final String? personaId;
  final String? proposalType;
  final String reviewState;
  final Map<String, dynamic> output;
  final List<MkProvenanceSource> sources;

  bool get isStale => reviewState == 'stale';
  bool get isApprovable =>
      schemaVersion == mkSchemaVersion2 &&
      reviewState == 'pending' &&
      !isStale &&
      targetEventId != null &&
      targetVersion != null;

  factory MkProposal.fromEvent(NostrEvent event) {
    final content = _decodeObject(event.content);
    final target = _optionalObject(content['target']);
    final output = _optionalObject(content['output']) ?? const {};
    final sources = (content['provenance'] as List<dynamic>? ?? const [])
        .map(MkProvenanceSource.tryParse)
        .whereType<MkProvenanceSource>()
        .toList(growable: false);
    final flatProvenance = (content['provenance'] as List<dynamic>? ?? const [])
        .whereType<String>()
        .toList(growable: false);
    return MkProposal(
      eventId: event.id,
      proposalId: _requiredString(content, 'proposal_id'),
      targetId: target == null
          ? _requiredString(content, 'target_id')
          : _requiredString(target, 'id'),
      targetKind: target == null
          ? _requiredInt(content, 'target_kind')
          : _requiredInt(target, 'kind'),
      agent:
          _optionalString(content, ['agent', 'persona', 'persona_id']) ??
          'MK agent',
      summary:
          _optionalString(content, ['summary']) ?? 'Proposal ready for review',
      provenance: sources.isEmpty
          ? flatProvenance
          : sources.map((source) => source.title).toList(growable: false),
      clips:
          (output['clips'] as List<dynamic>? ??
                  content['clips'] as List<dynamic>? ??
                  const [])
              .whereType<Map<String, dynamic>>()
              .map(Map<String, dynamic>.unmodifiable)
              .toList(growable: false),
      createdAt: event.createdAt,
      schemaVersion:
          _optionalInt(content, 'schema_version') ?? mkSchemaVersion1,
      proposalVersion:
          _optionalInt(content, 'proposal_version') ??
          _optionalInt(content, 'version') ??
          1,
      targetEventId: target == null
          ? _optionalString(content, ['target_event_id'])
          : _optionalString(target, ['event_id']),
      targetVersion: target == null
          ? _optionalInt(content, 'target_version')
          : _optionalInt(target, 'version'),
      runId: _optionalString(content, ['run_id']),
      provider: _optionalString(content, ['provider']),
      model: _optionalString(content, ['model']),
      outcome: _optionalString(content, ['status', 'outcome']) ?? 'proposed',
      personaId: _optionalString(content, ['persona_id']),
      proposalType: _optionalString(content, ['proposal_type']),
      reviewState:
          _optionalString(content, ['review_state']) ??
          (content['stale'] is Map ? 'stale' : 'pending'),
      output: Map.unmodifiable(output),
      sources: List.unmodifiable(sources),
    );
  }
}

@immutable
class MkProvenanceSource {
  const MkProvenanceSource({
    required this.sourceId,
    required this.sourceType,
    required this.title,
    required this.locator,
    this.sha256,
    this.retrievedAt,
  });

  final String sourceId;
  final String sourceType;
  final String title;
  final String locator;
  final String? sha256;
  final String? retrievedAt;

  static MkProvenanceSource? tryParse(Object? value) {
    final source = _optionalObject(value);
    if (source == null) return null;
    final sourceId = _optionalString(source, ['source_id']);
    final sourceType = _optionalString(source, ['source_type']);
    final title = _optionalString(source, ['title']);
    final locator = _optionalString(source, ['locator']);
    if (sourceId == null ||
        sourceType == null ||
        title == null ||
        locator == null) {
      return null;
    }
    return MkProvenanceSource(
      sourceId: sourceId,
      sourceType: sourceType,
      title: title,
      locator: locator,
      sha256: _optionalString(source, ['sha256']),
      retrievedAt: _optionalString(source, ['retrieved_at']),
    );
  }
}

@immutable
class MkApprovalAction {
  const MkApprovalAction({
    required this.eventId,
    required this.createdAt,
    required this.approvalId,
    required this.proposalId,
    required this.decision,
    this.actionId,
    this.approvalEventId,
    this.proposalEventId,
    this.targetId,
    this.targetKind,
    this.targetEventId,
    this.targetVersion,
    this.reason,
  });

  final String eventId;
  final int createdAt;
  final String approvalId;
  final String proposalId;
  final MkApprovalStatus decision;
  final String? actionId;
  final String? approvalEventId;
  final String? proposalEventId;
  final String? targetId;
  final int? targetKind;
  final String? targetEventId;
  final int? targetVersion;
  final String? reason;

  factory MkApprovalAction.fromEvent(NostrEvent event) {
    final content = _decodeObject(event.content);
    final decision = _requiredString(content, 'decision');
    if (decision != 'approved' && decision != 'rejected') {
      throw const FormatException('Approval action must approve or reject');
    }
    return MkApprovalAction(
      eventId: event.id,
      createdAt: event.createdAt,
      approvalId: _requiredString(content, 'approval_id'),
      proposalId: _requiredString(content, 'proposal_id'),
      decision: decision == 'approved'
          ? MkApprovalStatus.approved
          : MkApprovalStatus.rejected,
      actionId: _optionalString(content, ['action_id']),
      approvalEventId: _optionalString(content, ['approval_event_id']),
      proposalEventId: _optionalString(content, ['proposal_event_id']),
      targetId: _optionalString(content, ['target_id']),
      targetKind: _optionalInt(content, 'target_kind'),
      targetEventId: _optionalString(content, ['target_event_id']),
      targetVersion: _optionalInt(content, 'target_version'),
      reason: _optionalString(content, ['reason']),
    );
  }
}

enum MkCaptureType {
  guest('Guest', MkEntityType.person),
  task('Task', MkEntityType.task),
  meeting('Meeting', MkEntityType.meeting),
  contentIdea('Content idea', MkEntityType.content),
  knowledgeNote('Knowledge note', MkEntityType.knowledge);

  const MkCaptureType(this.label, this.entityType);
  final String label;
  final MkEntityType entityType;
}

@immutable
class MkCaptureDraft {
  const MkCaptureDraft({
    required this.type,
    required this.title,
    this.details = '',
    this.organization = '',
    this.whyNow = '',
    this.contextFields = const {},
  });

  final MkCaptureType type;
  final String title;
  final String details;
  final String organization;
  final String whyNow;
  final Map<String, dynamic> contextFields;
}

MkEntityData _parseEntityData(
  MkEntityType type,
  Map<String, dynamic> content,
  String rawStatus,
) => switch (type) {
  MkEntityType.goal => MkGoalData(
    content,
    _enumStatus(MkGoalStatus.values, rawStatus, type),
  ),
  MkEntityType.operationalProject => MkProjectData(
    content,
    _enumStatus(MkProjectStatus.values, rawStatus, type),
  ),
  MkEntityType.task => MkTaskData(
    content,
    _enumStatus(MkTaskStatus.values, _legacyStatus(type, rawStatus), type),
  ),
  MkEntityType.person => MkPersonData(
    content,
    _enumStatus(MkPersonStatus.values, _legacyStatus(type, rawStatus), type),
  ),
  MkEntityType.interview => MkInterviewData(
    content,
    _enumStatus(MkInterviewStatus.values, _legacyStatus(type, rawStatus), type),
  ),
  MkEntityType.content => MkContentData(
    content,
    _enumStatus(MkContentStatus.values, _legacyStatus(type, rawStatus), type),
  ),
  MkEntityType.meeting => MkMeetingData(
    content,
    _enumStatus(MkMeetingStatus.values, rawStatus, type),
  ),
  MkEntityType.decision => MkDecisionData(
    content,
    _enumStatus(MkDecisionStatus.values, rawStatus, type),
  ),
  MkEntityType.knowledge => MkKnowledgeData(
    content,
    _enumStatus(MkKnowledgeStatus.values, rawStatus, type),
  ),
  MkEntityType.approval => MkApprovalData(
    content,
    _enumStatus(MkApprovalStatus.values, rawStatus, type),
  ),
};

T _enumStatus<T extends MkStatusValue>(
  List<T> values,
  String wireName,
  MkEntityType type,
) {
  for (final value in values) {
    if (value.wireName == wireName) return value;
  }
  throw FormatException('Unsupported ${type.wireName} status $wireName');
}

String _legacyStatus(MkEntityType type, String value) => switch (type) {
  MkEntityType.person => switch (value) {
    'potential' => 'prospect',
    'research_ready' ||
    'interview_planning' ||
    'outreach_drafted' ||
    'awaiting_approval' => 'ready-to-contact',
    'follow_up_due' => 'contacted',
    'interested' => 'responded',
    'scheduling' => 'scheduled',
    'content_processing' => 'interviewed',
    'relationship_nurture' || 'paused' => 'nurture',
    'declined' || 'no_response' || 'not_a_fit' => 'closed',
    'do_not_contact' => 'archived',
    _ => value,
  },
  MkEntityType.interview => switch (value) {
    'research' || 'questions_ready' || 'awaiting_review' => 'planning',
    'transcript_processing' => 'transcribing',
    'content_processing' || 'review' => 'reviewing',
    'archived' => 'complete',
    _ => value,
  },
  MkEntityType.content => switch (value) {
    'research' || 'planned' || 'recording' || 'editing' => 'draft',
    'internal_review' ||
    'changes_requested' ||
    'performance_review' => 'in-review',
    _ => value,
  },
  MkEntityType.task => switch (value) {
    'open' => 'to-do',
    'in_progress' => 'in-progress',
    'waiting' => 'blocked',
    'complete' => 'done',
    _ => value,
  },
  _ => value,
};

Map<String, dynamic> _decodeObject(String content) {
  final decoded = jsonDecode(content);
  if (decoded is! Map<String, dynamic>) {
    throw const FormatException('MK content must be a JSON object');
  }
  return Map<String, dynamic>.unmodifiable(decoded);
}

String _requiredString(Map<String, dynamic> value, String key) {
  final field = value[key];
  if (field is! String || field.trim().isEmpty) {
    throw FormatException('$key must be a non-empty string');
  }
  return field;
}

int _requiredInt(Map<String, dynamic> value, String key) {
  final field = value[key];
  if (field is! int) throw FormatException('$key must be an integer');
  return field;
}

int? _optionalInt(Map<String, dynamic> value, String key) {
  final field = value[key];
  return field is int ? field : null;
}

Map<String, dynamic>? _optionalObject(Object? value) =>
    value is Map<String, dynamic> ? value : null;

List<String> _stringList(Object? value) => value is List
    ? value
          .whereType<String>()
          .map((item) => item.trim())
          .where((item) => item.isNotEmpty)
          .toList(growable: false)
    : const [];

String? _optionalString(Map<String, dynamic> value, List<String> keys) {
  for (final key in keys) {
    final field = value[key];
    if (field is String && field.trim().isNotEmpty) return field.trim();
  }
  return null;
}

String _recordTitle(Map<String, dynamic> raw, {required String fallback}) =>
    _optionalString(raw, ['name', 'title']) ?? fallback;

bool _isUuid(String value) => RegExp(
  r'^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[1-8][0-9a-fA-F]{3}-[89abAB][0-9a-fA-F]{3}-[0-9a-fA-F]{12}$',
).hasMatch(value);
