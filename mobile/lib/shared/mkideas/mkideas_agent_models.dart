import 'dart:convert';

import 'package:flutter/foundation.dart';

import '../relay/nostr_models.dart';

/// The five human-gated MK Ideas agent roles.
enum MkAgentPersona {
  guestResearcher(
    'guest-researcher',
    'Guest Researcher',
    'Builds sourced guest research drafts.',
  ),
  outreachDrafter(
    'outreach-drafter',
    'Outreach Drafter',
    'Drafts outreach but can never send it.',
  ),
  interviewProducer(
    'interview-producer',
    'Interview Producer',
    'Proposes interview preparation and questions.',
  ),
  contentClipCopilot(
    'content-clip-copilot',
    'Content / Clip Copilot',
    'Proposes transcript-grounded clips and captions.',
  ),
  operationsBriefingAssistant(
    'operations-briefing-assistant',
    'Operations Briefing Assistant',
    'Summarizes Today without changing operational state.',
  );

  const MkAgentPersona(this.id, this.label, this.purpose);

  final String id;
  final String label;
  final String purpose;

  bool get draftOnly => this != operationsBriefingAssistant;
  bool get informationalOnly => this == operationsBriefingAssistant;

  static MkAgentPersona? fromId(String? id) {
    for (final persona in values) {
      if (persona.id == id) return persona;
    }
    return null;
  }
}

enum MkAgentRunStatus {
  queued('queued', 'Queued'),
  running('running', 'Running'),
  retrying('retrying', 'Retrying'),
  succeeded('succeeded', 'Complete'),
  failed('failed', 'Failed'),
  cancelled('cancelled', 'Cancelled'),
  timedOut('timed_out', 'Timed out');

  const MkAgentRunStatus(this.wireName, this.label);

  final String wireName;
  final String label;

  bool get isFailure => this == failed || this == cancelled || this == timedOut;

  static MkAgentRunStatus parse(String value) {
    for (final status in values) {
      if (status.wireName == value) return status;
    }
    throw FormatException('Unsupported agent run status $value');
  }
}

@immutable
class MkAgentTarget {
  const MkAgentTarget({
    required this.kind,
    required this.entityId,
    required this.eventId,
    required this.version,
  });

  final int kind;
  final String entityId;
  final String eventId;
  final int version;

  factory MkAgentTarget.fromJson(Map<String, dynamic> value) => MkAgentTarget(
    kind: _requiredInt(value, 'kind'),
    entityId: _requiredString(value, 'id'),
    eventId: _requiredString(value, 'event_id'),
    version: _requiredInt(value, 'version'),
  );
}

@immutable
class MkAgentRun {
  const MkAgentRun({
    required this.eventId,
    required this.createdAt,
    required this.runId,
    required this.persona,
    required this.status,
    required this.attempt,
    this.target,
    this.startedAt,
    this.completedAt,
    this.errorCode,
    this.errorMessage,
    this.retryable = false,
  });

  final String eventId;
  final int createdAt;
  final String runId;
  final MkAgentPersona persona;
  final MkAgentRunStatus status;
  final int attempt;
  final MkAgentTarget? target;
  final String? startedAt;
  final String? completedAt;
  final String? errorCode;
  final String? errorMessage;
  final bool retryable;

  String get identity => '$runId:$attempt:${status.wireName}';

  factory MkAgentRun.fromEvent(NostrEvent event) {
    final content = _decodeObject(event.content);
    if (content['activity_type'] != 'agent_run') {
      throw const FormatException('System activity is not an agent run');
    }
    final persona = MkAgentPersona.fromId(
      _requiredString(content, 'persona_id'),
    );
    if (persona == null) {
      throw const FormatException('Unsupported MK agent persona');
    }
    final targetValue = content['target'];
    final errorValue = content['error'];
    final error = errorValue is Map<String, dynamic> ? errorValue : null;
    return MkAgentRun(
      eventId: event.id,
      createdAt: event.createdAt,
      runId: _requiredString(content, 'run_id'),
      persona: persona,
      status: MkAgentRunStatus.parse(_requiredString(content, 'status')),
      attempt: _requiredInt(content, 'attempt'),
      target: targetValue is Map<String, dynamic>
          ? MkAgentTarget.fromJson(targetValue)
          : null,
      startedAt: _optionalString(content, 'started_at'),
      completedAt: _optionalString(content, 'completed_at'),
      errorCode: error == null ? null : _optionalString(error, 'code'),
      errorMessage: error == null ? null : _optionalString(error, 'message'),
      retryable: error?['retryable'] == true,
    );
  }
}

@immutable
class MkGeneratedSummary {
  const MkGeneratedSummary({
    required this.eventId,
    required this.createdAt,
    required this.summaryId,
    required this.version,
    required this.persona,
    required this.output,
    this.runId,
  });

  final String eventId;
  final int createdAt;
  final String summaryId;
  final int version;
  final MkAgentPersona persona;
  final Map<String, dynamic> output;
  final String? runId;

  List<String> items(String key) => switch (output[key]) {
    final List<dynamic> values => values.whereType<String>().toList(
      growable: false,
    ),
    _ => const [],
  };

  factory MkGeneratedSummary.fromEvent(NostrEvent event) {
    final content = _decodeObject(event.content);
    final persona = MkAgentPersona.fromId(
      _requiredString(content, 'persona_id'),
    );
    if (persona != MkAgentPersona.operationsBriefingAssistant) {
      throw const FormatException('Generated summary has invalid persona');
    }
    final output = content['output'];
    if (output is! Map<String, dynamic>) {
      throw const FormatException('Generated summary output is required');
    }
    return MkGeneratedSummary(
      eventId: event.id,
      createdAt: event.createdAt,
      summaryId: _requiredString(content, 'summary_id'),
      version: _requiredInt(content, 'summary_version'),
      persona: persona!,
      output: Map.unmodifiable(output),
      runId: _optionalString(content, 'run_id'),
    );
  }
}

Map<String, dynamic> _decodeObject(String content) {
  final decoded = jsonDecode(content);
  if (decoded is! Map<String, dynamic>) {
    throw const FormatException('Agent event content must be an object');
  }
  return decoded;
}

String _requiredString(Map<String, dynamic> value, String key) {
  final field = value[key];
  if (field is! String || field.trim().isEmpty) {
    throw FormatException('$key must be a non-empty string');
  }
  return field.trim();
}

String? _optionalString(Map<String, dynamic> value, String key) {
  final field = value[key];
  return field is String && field.trim().isNotEmpty ? field.trim() : null;
}

int _requiredInt(Map<String, dynamic> value, String key) {
  final field = value[key];
  if (field is! int) throw FormatException('$key must be an integer');
  return field;
}
