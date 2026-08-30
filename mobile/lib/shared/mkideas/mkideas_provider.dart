import 'dart:async';
import 'dart:convert';

import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:uuid/uuid.dart';

import '../relay/nostr_models.dart';
import '../relay/relay_provider.dart';
import '../relay/relay_session.dart';
import '../relay/signed_event_relay.dart';

const _uuid = Uuid();
const _v0Kinds = [
  EventKind.mkPerson,
  EventKind.mkInterview,
  EventKind.mkContent,
  EventKind.mkApproval,
  EventKind.mkApprovalAction,
  EventKind.mkAgentProposal,
];

String mkCommunityHost(String relayUrl) {
  final uri = Uri.parse(relayUrl);
  final host = uri.host.toLowerCase().replaceFirst(RegExp(r'\.$'), '');
  final defaultPort =
      (uri.scheme == 'wss' || uri.scheme == 'https') && uri.port == 443 ||
      (uri.scheme == 'ws' || uri.scheme == 'http') && uri.port == 80;
  return uri.hasPort && !defaultPort ? '$host:${uri.port}' : host;
}

class MkRecord {
  final String eventId;
  final String author;
  final int kind;
  final int createdAt;
  final String entityId;
  final int version;
  final String recordType;
  final String status;
  final Map<String, dynamic> data;

  const MkRecord({
    required this.eventId,
    required this.author,
    required this.kind,
    required this.createdAt,
    required this.entityId,
    required this.version,
    required this.recordType,
    required this.status,
    required this.data,
  });

  String get title =>
      (data['name'] as String?)?.trim().isNotEmpty == true
      ? data['name'] as String
      : (data['title'] as String?) ?? 'Untitled record';
}

class MkProposal {
  final String eventId;
  final String proposalId;
  final String targetId;
  final int targetKind;
  final String agent;
  final String summary;
  final List<String> provenance;
  final List<Map<String, dynamic>> clips;

  const MkProposal({
    required this.eventId,
    required this.proposalId,
    required this.targetId,
    required this.targetKind,
    required this.agent,
    required this.summary,
    required this.provenance,
    required this.clips,
  });
}

class MkIdeasSnapshot {
  final List<MkRecord> records;
  final List<MkProposal> proposals;

  const MkIdeasSnapshot({required this.records, required this.proposals});

  static const empty = MkIdeasSnapshot(records: [], proposals: []);
}

MkIdeasSnapshot _parseEvents(List<NostrEvent> events) {
  final records = <MkRecord>[];
  final proposals = <MkProposal>[];
  for (final event in events) {
    final dynamic decoded;
    try {
      decoded = jsonDecode(event.content);
    } on FormatException {
      continue;
    }
    if (decoded is! Map<String, dynamic>) continue;
    if (event.kind >= EventKind.mkPerson && event.kind <= EventKind.mkApproval) {
      final entityId = decoded['entity_id'];
      final version = decoded['version'];
      if (entityId is! String || version is! int) continue;
      records.add(
        MkRecord(
          eventId: event.id,
          author: event.pubkey,
          kind: event.kind,
          createdAt: event.createdAt,
          entityId: entityId,
          version: version,
          recordType: decoded['record_type'] as String? ?? '',
          status: decoded['status'] as String? ?? '',
          data: decoded,
        ),
      );
    } else if (event.kind == EventKind.mkAgentProposal) {
      proposals.add(
        MkProposal(
          eventId: event.id,
          proposalId: decoded['proposal_id'] as String? ?? '',
          targetId: decoded['target_id'] as String? ?? '',
          targetKind: decoded['target_kind'] as int? ?? 0,
          agent: decoded['agent'] as String? ?? 'MK agent',
          summary: decoded['summary'] as String? ?? 'Proposal ready for review',
          provenance: (decoded['provenance'] as List<dynamic>? ?? const [])
              .whereType<String>()
              .toList(),
          clips: (decoded['clips'] as List<dynamic>? ?? const [])
              .whereType<Map<String, dynamic>>()
              .toList(),
        ),
      );
    }
  }
  records.sort((a, b) => b.createdAt.compareTo(a.createdAt));
  return MkIdeasSnapshot(records: records, proposals: proposals);
}

class MkIdeasNotifier extends AsyncNotifier<MkIdeasSnapshot> {
  void Function()? _unsubscribe;

  @override
  Future<MkIdeasSnapshot> build() async {
    final config = ref.watch(relayConfigProvider);
    ref.onDispose(() => _unsubscribe?.call());
    final snapshot = await _fetch(config);
    unawaited(_subscribe(config));
    return snapshot;
  }

  Future<MkIdeasSnapshot> _fetch(RelayConfig config) async {
    final events = await ref.read(relaySessionProvider.notifier).queryRelay([
      NostrFilter(
        kinds: _v0Kinds,
        tags: {
          '#h': [mkCommunityHost(config.wsUrl)],
        },
        limit: 500,
      ),
    ]);
    return _parseEvents(events);
  }

  Future<void> _subscribe(RelayConfig config) async {
    _unsubscribe?.call();
    try {
      _unsubscribe = await ref.read(relaySessionProvider.notifier).subscribe(
        NostrFilter(
          kinds: _v0Kinds,
          tags: {
            '#h': [mkCommunityHost(config.wsUrl)],
          },
          since: DateTime.now().millisecondsSinceEpoch ~/ 1000,
          limit: 0,
        ),
        (_) => ref.invalidateSelf(),
      );
    } catch (_) {
      // The HTTP snapshot remains useful while the session reconnects.
    }
  }

  Future<NostrEvent> _publishState({
    required int kind,
    required String recordType,
    required String status,
    required Map<String, dynamic> fields,
    String? entityId,
    MkRecord? previous,
  }) async {
    final config = ref.read(relayConfigProvider);
    final id = entityId ?? previous?.entityId ?? _uuid.v4();
    final version = (previous?.version ?? 0) + 1;
    final tags = <List<String>>[
      ['d', id],
      ['h', mkCommunityHost(config.wsUrl)],
      ['version', '$version'],
      ['status', status],
      if (previous != null) ['prev', previous.eventId],
    ];
    if (kind == EventKind.mkInterview && fields['guest_id'] is String) {
      tags.add(['guest', fields['guest_id'] as String]);
    }
    if (kind == EventKind.mkContent && fields['interview_id'] is String) {
      tags.add(['interview', fields['interview_id'] as String]);
    }
    if (kind == EventKind.mkApproval &&
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
    final relay = SignedEventRelay(
      session: ref.read(relaySessionProvider.notifier),
      nsec: config.nsec,
    );
    final result = await relay.submit(
      kind: kind,
      tags: tags,
      content: jsonEncode({
        ...fields,
        'schema_version': 1,
        'record_type': recordType,
        'entity_id': id,
        'version': version,
        'status': status,
      }),
    );
    return result;
  }

  Future<void> addGuest({
    required String name,
    required String organization,
    required String whyNow,
  }) async {
    await _publishState(
      kind: EventKind.mkPerson,
      recordType: 'person',
      status: 'potential',
      fields: {
        'name': name.trim(),
        'organization': organization.trim(),
        'why_now': whyNow.trim(),
        'do_not_contact': false,
      },
    );
    ref.invalidateSelf();
  }

  Future<void> createInterview(MkRecord guest) async {
    await _publishState(
      kind: EventKind.mkInterview,
      recordType: 'interview',
      status: 'planning',
      fields: {'title': 'Interview with ${guest.title}', 'guest_id': guest.entityId},
    );
    ref.invalidateSelf();
  }

  Future<void> attachTranscript(MkRecord interview, String name, String text) async {
    await _publishState(
      kind: EventKind.mkInterview,
      recordType: 'interview',
      status: 'content_processing',
      previous: interview,
      fields: {...interview.data, 'transcript_name': name, 'transcript_text': text},
    );
    ref.invalidateSelf();
  }

  Future<void> createContent(MkRecord interview) async {
    await _publishState(
      kind: EventKind.mkContent,
      recordType: 'content',
      status: 'internal_review',
      fields: {
        'title': '${interview.title} — clips',
        'interview_id': interview.entityId,
      },
    );
    ref.invalidateSelf();
  }

  Future<void> reviewProposal(MkProposal proposal, String decision) async {
    final config = ref.read(relayConfigProvider);
    final approvalId = proposal.proposalId;
    await _publishState(
      kind: EventKind.mkApproval,
      recordType: 'approval',
      status: decision,
      entityId: approvalId,
      fields: {
        'target_id': proposal.targetId,
        'target_kind': proposal.targetKind,
        'proposal_id': proposal.proposalId,
        'proposal_event_id': proposal.eventId,
      },
    );
    final relay = SignedEventRelay(
      session: ref.read(relaySessionProvider.notifier),
      nsec: config.nsec,
    );
    await relay.submit(
      kind: EventKind.mkApprovalAction,
      tags: [
        ['h', mkCommunityHost(config.wsUrl)],
        ['e', proposal.eventId],
      ],
      content: jsonEncode({
        'schema_version': 1,
        'approval_id': approvalId,
        'target_id': proposal.targetId,
        'proposal_id': proposal.proposalId,
        'decision': decision,
      }),
    );
    ref.invalidateSelf();
  }
}

final mkIdeasProvider = AsyncNotifierProvider<MkIdeasNotifier, MkIdeasSnapshot>(
  MkIdeasNotifier.new,
);
