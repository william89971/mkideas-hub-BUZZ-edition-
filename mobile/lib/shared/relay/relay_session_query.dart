part of 'relay_session.dart';

Future<List<NostrEvent>> _queryRelayEvents(
  RelaySessionNotifier session,
  List<NostrFilter> filters, {
  required Duration timeout,
}) async {
  final decoded = await _queryRelayJson(
    session,
    filters.map((filter) => filter.toJson()).toList(),
    timeout: timeout,
  );
  if (decoded is! List) {
    throw const FormatException('relay returned malformed query response');
  }
  return _decodeQueryEvents(decoded);
}

Future<RelayQueryPage> _queryRelayPage(
  RelaySessionNotifier session,
  NostrFilter filter, {
  required Duration timeout,
}) async {
  final decoded = await _queryRelayJson(session, [
    filter.toJson(),
  ], timeout: timeout);
  if (decoded is! Map<String, dynamic>) {
    throw const FormatException('relay returned malformed projection response');
  }
  final rawEvents = decoded['events'];
  final rawCursor = decoded['nextCursor'];
  if (rawEvents is! List || rawCursor != null && rawCursor is! String) {
    throw const FormatException('relay returned malformed projection page');
  }
  return RelayQueryPage(
    events: _decodeQueryEvents(rawEvents),
    nextCursor: rawCursor as String?,
  );
}

Future<dynamic> _queryRelayJson(
  RelaySessionNotifier session,
  Object body, {
  required Duration timeout,
}) async {
  final config = session._activeRelayConfig;
  final url = Uri.parse(config.baseUrl).resolve('/query').toString();
  final bodyBytes = utf8.encode(jsonEncode(body));
  final response = await session._httpQueryClient.post(
    Uri.parse(url),
    headers: {
      'Authorization': buildNip98AuthHeader(
        method: 'POST',
        url: url,
        bodyBytes: bodyBytes,
        nsec: config.nsec,
      ),
      'Content-Type': 'application/json',
    },
    body: bodyBytes,
    timeout: timeout,
  );
  if (response.statusCode < 200 || response.statusCode >= 300) {
    session._activateRateLimitGateFromHttpError(response.body);
    throw RelayException(response.statusCode, response.body);
  }
  return jsonDecode(response.body);
}

List<NostrEvent> _decodeQueryEvents(List<dynamic> decoded) {
  try {
    return [
      for (final eventJson in decoded)
        if (eventJson is Map<String, dynamic>)
          NostrEvent.fromJson(eventJson)
        else
          throw const FormatException('relay returned malformed query event'),
    ];
  } catch (error) {
    if (error is FormatException) rethrow;
    throw FormatException('relay returned malformed query event: $error');
  }
}
