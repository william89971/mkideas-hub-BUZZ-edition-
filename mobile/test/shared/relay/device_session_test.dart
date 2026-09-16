import 'dart:convert';

import 'package:buzz/shared/relay/device_session.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nostr/nostr.dart' as nostr;

class MemorySecureStorage extends Fake implements FlutterSecureStorage {
  final values = <String, String>{};

  @override
  Future<String?> read({
    required String key,
    AppleOptions? iOptions,
    AndroidOptions? aOptions,
    LinuxOptions? lOptions,
    WebOptions? webOptions,
    AppleOptions? mOptions,
    WindowsOptions? wOptions,
  }) async => values[key];
}

void main() {
  test(
    'enrollment proof is scoped to identity, relay, and fresh challenge',
    () async {
      final storage = MemorySecureStorage();
      final human = nostr.Keys.generate();
      final device = nostr.Keys.generate();
      const relay = 'wss://hub.example.test';
      storage.values['device-key'] = device.nsec;
      storage.values[deviceSessionStorageKey(
        human.public,
        relay,
      )] = jsonEncode({
        'human_pubkey': human.public,
        'relay_url': relay,
        'community_id': 'ce0a5c0b-e14e-4c17-9b5b-e74272349ceb',
        'grant_id': 'd25c024d-9ad7-43d2-9849-d4c02a4e6200',
        'key_storage': 'device-key',
      });
      final tag = await deviceSessionTag(
        human: human.public,
        relay: '$relay/',
        challenge: 'fresh-login',
        secure: storage,
      );
      expect(tag!.take(3), [
        'mk-device',
        'd25c024d-9ad7-43d2-9849-d4c02a4e6200',
        device.public,
      ]);
      final proof = jsonDecode(tag[3]) as Map<String, dynamic>;
      expect(proof['pubkey'], device.public);
      expect(proof['kind'], 22242);
      final payload =
          jsonDecode(proof['content'] as String) as Map<String, dynamic>;
      expect(
        payload['challenge_hash'],
        deviceSessionHash(utf8.encode('fresh-login')),
      );
      expect(payload['human_pubkey'], human.public);
      expect(payload['relay_url'], relay);
      expect(
        await deviceSessionTag(
          human: human.public,
          relay: 'wss://other.test',
          challenge: 'fresh-login',
          secure: storage,
        ),
        isNull,
      );
      expect(
        await deviceSessionTag(
          human: device.public,
          relay: relay,
          challenge: 'fresh-login',
          secure: storage,
        ),
        isNull,
      );
      storage.values.remove('device-key');
      await expectLater(
        deviceSessionTag(
          human: human.public,
          relay: relay,
          challenge: 'fresh-login',
          secure: storage,
        ),
        throwsStateError,
      );
    },
  );
}
