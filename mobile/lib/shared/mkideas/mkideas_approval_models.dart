import 'package:flutter/foundation.dart';

import '../relay/nostr_models.dart';
import 'mkideas_models.dart';

/// Exact client payload for the relay's one-transaction approval path.
@immutable
class MkAtomicApprovalDecision {
  const MkAtomicApprovalDecision({
    required this.actionId,
    required this.approvalId,
    required this.approvalEventId,
    required this.targetId,
    required this.targetKind,
    required this.targetEventId,
    required this.targetVersion,
    required this.proposalId,
    required this.proposalEventId,
    required this.decision,
    required this.reason,
    this.resultEvent,
  });

  final String actionId;
  final String approvalId;
  final String approvalEventId;
  final String targetId;
  final int targetKind;
  final String targetEventId;
  final int targetVersion;
  final String proposalId;
  final String proposalEventId;
  final String decision;
  final String reason;
  final NostrEvent? resultEvent;

  Map<String, dynamic> toContent() {
    if (decision != 'approved' && decision != 'rejected') {
      throw ArgumentError.value(decision, 'decision');
    }
    if (decision == 'rejected' && resultEvent != null) {
      throw ArgumentError('A rejected action cannot carry a result event.');
    }
    if (reason.trim().isEmpty) {
      throw ArgumentError.value(reason, 'reason', 'must not be empty');
    }
    return {
      'schema_version': mkSchemaVersion2,
      'action_id': actionId,
      'approval_id': approvalId,
      'approval_event_id': approvalEventId,
      'target_id': targetId,
      'target_kind': targetKind,
      'target_event_id': targetEventId,
      'target_version': targetVersion,
      'proposal_id': proposalId,
      'proposal_event_id': proposalEventId,
      'decision': decision,
      'reason': reason.trim(),
      if (resultEvent != null) 'result_event': resultEvent!.toJson(),
    };
  }
}
