import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:http/http.dart' as http;
import 'package:nostr/nostr.dart' as nostr;
import 'package:pointycastle/digests/sha256.dart';
import 'package:uuid/uuid.dart';

import '../relay/relay_provider.dart';
import '../relay/device_session.dart';

class MkDeviceGrant {
  const MkDeviceGrant({
    required this.id,
    required this.devicePubkey,
    required this.deviceName,
    required this.platform,
    required this.issuedAt,
    required this.lastSeenAt,
    required this.revokedAt,
  });

  final String id;
  final String devicePubkey;
  final String deviceName;
  final String platform;
  final DateTime issuedAt;
  final DateTime? lastSeenAt;
  final DateTime? revokedAt;

  factory MkDeviceGrant.fromJson(Map<String, dynamic> json) => MkDeviceGrant(
    id: json['id'] as String,
    devicePubkey: json['device_pubkey'] as String,
    deviceName: json['device_name'] as String,
    platform: json['platform'] as String,
    issuedAt: DateTime.parse(json['issued_at'] as String),
    lastSeenAt: json['last_seen_at'] == null
        ? null
        : DateTime.parse(json['last_seen_at'] as String),
    revokedAt: json['revoked_at'] == null
        ? null
        : DateTime.parse(json['revoked_at'] as String),
  );
}

class MkIdeasDeviceApi {
  MkIdeasDeviceApi({
    required this.config,
    FlutterSecureStorage? secureStorage,
    http.Client? client,
  }) : _secure = secureStorage ?? const FlutterSecureStorage(),
       _client = client ?? http.Client();

  final RelayConfig config;
  final FlutterSecureStorage _secure;
  final http.Client _client;

  String _sha256Hex(List<int> bytes) => SHA256Digest()
      .process(Uint8List.fromList(bytes))
      .map((byte) => byte.toRadixString(16).padLeft(2, '0'))
      .join();

  String get _humanPubkey {
    final nsec = config.nsec;
    final pubkey = pubkeyFromNsec(nsec);
    if (nsec == null || pubkey == null) {
      throw StateError('Device management requires a signing identity.');
    }
    return pubkey;
  }

  String get _deviceStorageKey =>
      'buzz_mk_device_v1_${_sha256Hex(utf8.encode('${config.baseUrl.toLowerCase()}:$_humanPubkey'))}';

  Future<nostr.Keys> _deviceKeys() async {
    final existing = await _secure.read(key: _deviceStorageKey);
    if (existing != null) {
      final decoded = nostr.Nip19.decode(payload: existing).data;
      if (decoded.isEmpty) throw StateError('Stored device key is invalid.');
      return nostr.Keys(decoded);
    }
    final keys = nostr.Keys.generate();
    await _secure.write(key: _deviceStorageKey, value: keys.nsec);
    final verified = await _secure.read(key: _deviceStorageKey);
    if (verified != keys.nsec) {
      await _secure.delete(key: _deviceStorageKey);
      throw StateError('Device key secure-storage verification failed.');
    }
    return keys;
  }

  String _authHeader(String method, String url, List<int>? body) {
    final nsec = config.nsec;
    if (nsec == null || nsec.isEmpty) {
      throw StateError('Device management requires a signing identity.');
    }
    final privateKey = nostr.Nip19.decode(payload: nsec).data;
    final tags = <List<String>>[
      ['u', url],
      ['method', method],
      if (body != null) ['payload', _sha256Hex(body)],
      ['nonce', const Uuid().v4()],
    ];
    final event = nostr.Event.from(
      kind: 27235,
      content: '',
      tags: tags,
      secretKey: privateKey,
      verify: false,
    );
    return 'Nostr ${base64.encode(utf8.encode(event.toJson()))}';
  }

  Future<Map<String, dynamic>> _request(
    String path, {
    required String method,
    Map<String, dynamic>? body,
  }) async {
    final uri = Uri.parse(config.baseUrl).resolve(path);
    final bodyBytes = body == null ? null : utf8.encode(jsonEncode(body));
    final response = await _client
        .send(
          http.Request(method, uri)
            ..headers.addAll({
              'Authorization': _authHeader(method, uri.toString(), bodyBytes),
              if (bodyBytes != null) 'Content-Type': 'application/json',
            })
            ..bodyBytes = bodyBytes ?? const [],
        )
        .timeout(const Duration(seconds: 15));
    final responseBody = await response.stream.bytesToString();
    final decoded = responseBody.isEmpty
        ? <String, dynamic>{}
        : jsonDecode(responseBody);
    if (response.statusCode < 200 || response.statusCode >= 300) {
      final message = decoded is Map<String, dynamic> ? decoded['error'] : null;
      throw StateError(
        message is String ? message : 'HTTP ${response.statusCode}',
      );
    }
    if (decoded is! Map<String, dynamic>) {
      throw const FormatException('Invalid device service response.');
    }
    return decoded;
  }

  Future<List<MkDeviceGrant>> listDevices() async {
    final result = await _request('/api/devices', method: 'GET');
    final devices = result['devices'];
    if (devices is! List) {
      throw const FormatException('Invalid device inventory.');
    }
    return devices
        .map(
          (item) =>
              MkDeviceGrant.fromJson(Map<String, dynamic>.from(item as Map)),
        )
        .toList(growable: false);
  }

  Future<void> enroll({
    required String deviceName,
    required String platform,
  }) async {
    final deviceKeys = await _deviceKeys();
    final challenge = await _request(
      '/api/devices/enrollment-challenges',
      method: 'POST',
      body: {'device_pubkey': deviceKeys.public},
    );
    final challengeText = challenge['challenge'] as String;
    final proof = nostr.Event.from(
      kind: 22242,
      content: jsonEncode({
        'version': 1,
        'action': 'enroll',
        'community_id': challenge['community_id'],
        'human_pubkey': challenge['human_pubkey'],
        'device_pubkey': challenge['device_pubkey'],
        'challenge_id': challenge['challenge_id'],
        'challenge_hash': _sha256Hex(utf8.encode(challengeText)),
        'relay_url': challenge['relay_url'],
      }),
      tags: const [],
      secretKey: deviceKeys.secret,
      verify: false,
    );
    final enrollment = await _request(
      '/api/devices/enroll',
      method: 'POST',
      body: {
        'challenge_id': challenge['challenge_id'],
        'challenge': challengeText,
        'device_name': deviceName.trim(),
        'platform': platform,
        'device_proof': proof.toMap(),
      },
    );
    final receiptKey = deviceSessionStorageKey(
      _humanPubkey,
      challenge['relay_url'] as String,
    );
    final receipt = jsonEncode({
      'human_pubkey': _humanPubkey,
      'relay_url': challenge['relay_url'],
      'community_id': challenge['community_id'],
      'grant_id': enrollment['grant_id'],
      'key_storage': _deviceStorageKey,
    });
    await _secure.write(key: receiptKey, value: receipt);
    if (await _secure.read(key: receiptKey) != receipt) {
      throw StateError(
        'Device enrollment receipt could not be verified in secure storage.',
      );
    }
  }

  Future<void> revoke(String grantId, String reason) async {
    await _request(
      '/api/devices/revoke',
      method: 'POST',
      body: {'grant_id': grantId, 'reason': reason.trim()},
    );
  }

  void close() => _client.close();
}
