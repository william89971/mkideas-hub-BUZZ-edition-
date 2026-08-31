import 'package:flutter/foundation.dart';

import 'nostr_models.dart';
import 'relay_socket.dart';

enum SessionStatus { disconnected, connecting, connected, reconnecting }

typedef RelaySocketFactory =
    RelaySocket Function({
      required String wsUrl,
      required String? nsec,
      required void Function(List<dynamic> message) onMessage,
      required void Function() onConnected,
      required void Function(Object? error) onDisconnected,
    });

@immutable
class SessionState {
  final SessionStatus status;
  final int reconnectAttempt;

  const SessionState({required this.status, this.reconnectAttempt = 0});
}

/// One cursor-bearing page returned by an extended relay `/query` projection.
@immutable
class RelayQueryPage {
  const RelayQueryPage({required this.events, this.nextCursor});

  /// Signed events in this page.
  final List<NostrEvent> events;

  /// Opaque server-issued cursor, or null when the projection is complete.
  final String? nextCursor;
}

/// Recovery lifecycle for a live relay subscription.
enum RelaySubscriptionStatus { ready, retrying }
