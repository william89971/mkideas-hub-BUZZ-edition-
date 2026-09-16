import 'dart:convert';

import 'package:shared_preferences/shared_preferences.dart';

import '../relay/nostr_models.dart';
import 'mkideas_models.dart';

const _storeVersion = 1;

/// One signed MK Ideas write retained until the relay accepts it or a human
/// explicitly resolves its conflict.
class MkOutboxEntry {
  const MkOutboxEntry({
    required this.id,
    required this.relayUrl,
    required this.event,
    required this.type,
    required this.status,
    required this.fields,
    required this.entityId,
    required this.createdAt,
    required this.updatedAt,
    this.previousEventId,
    this.attempts = 0,
    this.conflicted = false,
    this.error,
  });

  final String id;
  final String relayUrl;
  final NostrEvent event;
  final MkEntityType type;
  final String status;
  final Map<String, dynamic> fields;
  final String entityId;
  final String? previousEventId;
  final DateTime createdAt;
  final DateTime updatedAt;
  final int attempts;
  final bool conflicted;
  final String? error;

  MkOutboxEntry copyWith({
    NostrEvent? event,
    String? previousEventId,
    DateTime? updatedAt,
    int? attempts,
    bool? conflicted,
    String? error,
  }) => MkOutboxEntry(
    id: event?.id ?? id,
    relayUrl: relayUrl,
    event: event ?? this.event,
    type: type,
    status: status,
    fields: fields,
    entityId: entityId,
    previousEventId: previousEventId ?? this.previousEventId,
    createdAt: createdAt,
    updatedAt: updatedAt ?? this.updatedAt,
    attempts: attempts ?? this.attempts,
    conflicted: conflicted ?? this.conflicted,
    error: error ?? this.error,
  );

  Map<String, dynamic> toJson() => {
    'id': id,
    'relay_url': relayUrl,
    'event': event.toJson(),
    'type': type.wireName,
    'status': status,
    'fields': fields,
    'entity_id': entityId,
    'previous_event_id': previousEventId,
    'created_at': createdAt.toIso8601String(),
    'updated_at': updatedAt.toIso8601String(),
    'attempts': attempts,
    'conflicted': conflicted,
    'error': error,
  };

  factory MkOutboxEntry.fromJson(Map<String, dynamic> json) {
    final type = MkEntityType.fromWireName(json['type'] as String? ?? '');
    if (type == null) throw const FormatException('unknown MK entity type');
    return MkOutboxEntry(
      id: json['id'] as String,
      relayUrl: json['relay_url'] as String,
      event: NostrEvent.fromJson(
        Map<String, dynamic>.from(json['event'] as Map),
      ),
      type: type,
      status: json['status'] as String,
      fields: Map<String, dynamic>.from(json['fields'] as Map),
      entityId: json['entity_id'] as String,
      previousEventId: json['previous_event_id'] as String?,
      createdAt: DateTime.parse(json['created_at'] as String),
      updatedAt: DateTime.parse(json['updated_at'] as String),
      attempts: json['attempts'] as int? ?? 0,
      conflicted: json['conflicted'] as bool? ?? false,
      error: json['error'] as String?,
    );
  }
}

/// Durable, identity-scoped cache and signed-write outbox for MK Ideas.
class MkIdeasOfflineStore {
  const MkIdeasOfflineStore(this._prefs);

  final SharedPreferences _prefs;

  String _scope(String relayUrl, String humanPubkey) =>
      '${Uri.encodeComponent(relayUrl.toLowerCase())}:$humanPubkey';

  String _cacheKey(String relayUrl, String humanPubkey) =>
      'buzz.mkideas.cache.v1:${_scope(relayUrl, humanPubkey)}';

  String _outboxKey(String relayUrl, String humanPubkey) =>
      'buzz.mkideas.outbox.v1:${_scope(relayUrl, humanPubkey)}';

  Future<void> writeCache(
    String relayUrl,
    String humanPubkey,
    List<NostrEvent> events,
  ) async {
    await _prefs.setString(
      _cacheKey(relayUrl, humanPubkey),
      jsonEncode({
        'version': _storeVersion,
        'saved_at': DateTime.now().toUtc().toIso8601String(),
        'events': events.map((event) => event.toJson()).toList(),
      }),
    );
  }

  List<NostrEvent> readCache(String relayUrl, String humanPubkey) {
    try {
      final raw = _prefs.getString(_cacheKey(relayUrl, humanPubkey));
      if (raw == null) return const [];
      final decoded = jsonDecode(raw);
      if (decoded is! Map || decoded['version'] != _storeVersion) {
        return const [];
      }
      return (decoded['events'] as List)
          .map(
            (item) =>
                NostrEvent.fromJson(Map<String, dynamic>.from(item as Map)),
          )
          .toList(growable: false);
    } catch (_) {
      return const [];
    }
  }

  List<MkOutboxEntry> readOutbox(String relayUrl, String humanPubkey) {
    try {
      final raw = _prefs.getString(_outboxKey(relayUrl, humanPubkey));
      if (raw == null) return <MkOutboxEntry>[];
      final decoded = jsonDecode(raw);
      if (decoded is! Map || decoded['version'] != _storeVersion) {
        return <MkOutboxEntry>[];
      }
      return (decoded['items'] as List)
          .map(
            (item) =>
                MkOutboxEntry.fromJson(Map<String, dynamic>.from(item as Map)),
          )
          .toList(growable: true);
    } catch (_) {
      return <MkOutboxEntry>[];
    }
  }

  Future<void> put(
    String relayUrl,
    String humanPubkey,
    MkOutboxEntry entry,
  ) async {
    final items = readOutbox(relayUrl, humanPubkey);
    final index = items.indexWhere((candidate) => candidate.id == entry.id);
    if (index == -1) {
      items.add(entry);
    } else {
      items[index] = entry;
    }
    await _writeOutbox(relayUrl, humanPubkey, items);
  }

  Future<void> remove(String relayUrl, String humanPubkey, String id) async {
    final items = readOutbox(relayUrl, humanPubkey)
      ..removeWhere((entry) => entry.id == id);
    await _writeOutbox(relayUrl, humanPubkey, items);
  }

  Future<void> _writeOutbox(
    String relayUrl,
    String humanPubkey,
    List<MkOutboxEntry> items,
  ) async {
    final key = _outboxKey(relayUrl, humanPubkey);
    if (items.isEmpty) {
      await _prefs.remove(key);
      return;
    }
    await _prefs.setString(
      key,
      jsonEncode({
        'version': _storeVersion,
        'items': items.map((entry) => entry.toJson()).toList(),
      }),
    );
  }
}

bool isMkOutboxConflict(Object error) => RegExp(
  r'\bconflict\b|stale|superseded|version',
  caseSensitive: false,
).hasMatch(error.toString());
