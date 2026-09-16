import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:nostr/nostr.dart' as nostr;
import 'package:pointycastle/digests/sha256.dart';

String deviceSessionHash(List<int> bytes) => SHA256Digest()
    .process(Uint8List.fromList(bytes))
    .map((byte) => byte.toRadixString(16).padLeft(2, '0'))
    .join();

String deviceSessionStorageKey(String human, String relay) =>
    'buzz_mk_session_v1_${deviceSessionHash(utf8.encode('$human:${relay.replaceFirst(RegExp(r'/+$'), '')}'))}';

/// Load an enrollment receipt and sign this exact login challenge. Missing
/// enrolled keys fail closed; login never generates replacement device keys.
Future<List<String>?> deviceSessionTag({
  required String human,
  required String relay,
  required String challenge,
  FlutterSecureStorage secure = const FlutterSecureStorage(),
}) async {
  final raw = await secure.read(key: deviceSessionStorageKey(human, relay));
  if (raw == null) return null;
  final binding = jsonDecode(raw) as Map<String, dynamic>;
  if (binding['human_pubkey'] != human ||
      deviceSessionStorageKey(human, binding['relay_url'] as String) !=
          deviceSessionStorageKey(human, relay)) {
    throw StateError(
      'Device enrollment does not match this identity and relay.',
    );
  }
  final nsec = await secure.read(key: binding['key_storage'] as String);
  if (nsec == null) {
    throw StateError(
      'Enrolled device key is unavailable. Restore access before signing in.',
    );
  }
  final keys = nostr.Keys(nostr.Nip19.decode(payload: nsec).data);
  final proof = nostr.Event.from(
    kind: 22242,
    content: jsonEncode({
      'version': 1,
      'community_id': binding['community_id'],
      'human_pubkey': human,
      'device_pubkey': keys.public,
      'challenge_hash': deviceSessionHash(utf8.encode(challenge)),
      'relay_url': binding['relay_url'],
    }),
    tags: const [],
    secretKey: keys.secret,
  );
  return [
    'mk-device',
    binding['grant_id'] as String,
    keys.public,
    proof.toJson(),
  ];
}
