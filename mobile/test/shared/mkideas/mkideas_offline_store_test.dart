import 'package:flutter_test/flutter_test.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'package:buzz/shared/mkideas/mkideas_models.dart';
import 'package:buzz/shared/mkideas/mkideas_offline_store.dart';
import 'package:buzz/shared/relay/nostr_models.dart';

void main() {
  const relayUrl = 'wss://hub.mkideas.org';
  const pubkey = 'human-pubkey';
  const event = NostrEvent(
    id: 'event-id',
    pubkey: pubkey,
    createdAt: 10,
    kind: EventKind.mkTask,
    tags: [
      ['d', 'task-id'],
      ['h', 'hub.mkideas.org'],
      ['version', '1'],
      ['status', 'to-do'],
    ],
    content: '{"title":"Book the studio"}',
    sig: 'signature',
  );

  late MkIdeasOfflineStore store;

  setUp(() async {
    SharedPreferences.setMockInitialValues({});
    store = MkIdeasOfflineStore(await SharedPreferences.getInstance());
  });

  test('cache is scoped by relay and signing identity', () async {
    await store.writeCache(relayUrl, pubkey, [event]);

    expect(store.readCache(relayUrl, pubkey).single.id, event.id);
    expect(store.readCache(relayUrl, 'another-human'), isEmpty);
  });

  test('signed outbox entries survive until explicit removal', () async {
    final now = DateTime.utc(2026, 9, 16);
    final entry = MkOutboxEntry(
      id: event.id,
      relayUrl: relayUrl,
      event: event,
      type: MkEntityType.task,
      status: 'to-do',
      fields: const {'title': 'Book the studio'},
      entityId: 'task-id',
      createdAt: now,
      updatedAt: now,
      attempts: 1,
      error: 'Relay connection closed.',
    );

    await store.put(relayUrl, pubkey, entry);
    final restored = store.readOutbox(relayUrl, pubkey).single;
    expect(restored.event.id, event.id);
    expect(restored.fields['title'], 'Book the studio');

    await store.remove(relayUrl, pubkey, entry.id);
    expect(store.readOutbox(relayUrl, pubkey), isEmpty);
  });

  test('only stale version failures become conflicts', () {
    expect(isMkOutboxConflict(Exception('conflict: version is stale')), isTrue);
    expect(isMkOutboxConflict(Exception('Relay connection closed.')), isFalse);
  });
}
