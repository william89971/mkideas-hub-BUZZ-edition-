import 'dart:convert';

import 'package:buzz/shared/mkideas/mkideas_device_api.dart';
import 'package:buzz/shared/relay/device_session.dart';
import 'package:buzz/shared/relay/relay_provider.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:nostr/nostr.dart' as nostr;

import '../community/community_storage_test.dart' show FakeSecureStorage;

void main() {
  test(
    'enrollment persists a receipt that signs the subsequent login',
    () async {
      final human = nostr.Keys.generate();
      final storage = FakeSecureStorage();
      const relay = 'wss://hub.company.test';
      const community = 'ce0a5c0b-e14e-4c17-9b5b-e74272349ceb';
      const grant = 'd25c024d-9ad7-43d2-9849-d4c02a4e6200';
      String? device;
      final client = MockClient((request) async {
        final auth = nostr.Event.fromJson(
          utf8.decode(
            base64.decode(request.headers['Authorization']!.substring(6)),
          ),
        );
        expect(auth.pubkey, human.public);
        expect(
          auth.tags,
          contains(equals(['payload', deviceSessionHash(request.bodyBytes)])),
        );
        final body = jsonDecode(request.body) as Map<String, dynamic>;
        if (request.url.path.endsWith('enrollment-challenges')) {
          device = body['device_pubkey'] as String;
          expect(device, isNot(human.public));
          return http.Response(
            jsonEncode({
              'challenge_id': '2030a6ef-54c2-44c8-9d4a-a52110ed8b57',
              'challenge': 'single-use-enrollment',
              'community_id': community,
              'human_pubkey': human.public,
              'device_pubkey': device,
              'relay_url': relay,
            }),
            200,
          );
        }
        expect(request.url.path, '/api/devices/enroll');
        final proof = nostr.Event.fromMap(
          Map<String, dynamic>.from(body['device_proof'] as Map),
        );
        expect(proof.pubkey, device);
        expect(jsonDecode(proof.content)['action'], 'enroll');
        return http.Response(
          jsonEncode({'grant_id': grant, 'status': 'active'}),
          200,
        );
      });
      final api = MkIdeasDeviceApi(
        config: RelayConfig(baseUrl: relay, nsec: human.nsec),
        secureStorage: storage,
        client: client,
      );
      await api.enroll(deviceName: 'Test phone', platform: 'ios');
      final tag = await deviceSessionTag(
        human: human.public,
        relay: relay,
        challenge: 'login-challenge',
        secure: storage,
      );
      expect(tag!.take(3), ['mk-device', grant, device]);
      final proof = nostr.Event.fromJson(tag[3]);
      final payload = jsonDecode(proof.content);
      expect(payload['community_id'], community);
      expect(
        payload['challenge_hash'],
        deviceSessionHash(utf8.encode('login-challenge')),
      );
      api.close();
    },
  );
}
