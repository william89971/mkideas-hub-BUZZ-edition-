import '../../shared/mkideas/mkideas_provider.dart';

enum MkTodayGroup {
  judgment('Needs your judgment'),
  blockedAndDue('Blocked and due'),
  upcoming('Upcoming'),
  agentOutcomes('Agent outcomes');

  const MkTodayGroup(this.label);
  final String label;
}

class MkTodayItem {
  const MkTodayItem({
    required this.key,
    required this.group,
    required this.priority,
    required this.title,
    required this.detail,
    this.record,
    this.proposal,
    this.run,
    this.dueAt,
  });

  final String key;
  final MkTodayGroup group;
  final int priority;
  final String title;
  final String detail;
  final MkRecord? record;
  final MkProposal? proposal;
  final MkAgentRun? run;
  final DateTime? dueAt;
}

List<MkTodayItem> buildMkTodayItems(MkIdeasSnapshot snapshot, {DateTime? now}) {
  final effectiveNow = now ?? DateTime.now();
  final candidates = <MkTodayItem>[];
  for (final proposal in snapshot.pendingProposals) {
    candidates.add(
      MkTodayItem(
        key: 'proposal:${proposal.proposalId}',
        group: proposal.isStale
            ? MkTodayGroup.agentOutcomes
            : MkTodayGroup.judgment,
        priority: proposal.isStale ? 45 : 100,
        title: proposal.isStale ? 'Fresh agent run needed' : proposal.summary,
        detail: proposal.isStale
            ? 'The target changed after this draft was produced.'
            : '${proposal.agent} · Human approval required',
        proposal: proposal,
      ),
    );
  }
  for (final record in snapshot.records) {
    final due = _recordDate(record);
    if (_isBlocked(record)) {
      candidates.add(
        MkTodayItem(
          key: 'record:${record.kind}:${record.entityId}',
          group: MkTodayGroup.blockedAndDue,
          priority: 90,
          title: record.title,
          detail: '${record.type.label} is blocked',
          record: record,
          dueAt: due,
        ),
      );
      continue;
    }
    if (due != null &&
        due.isBefore(effectiveNow.add(const Duration(days: 3)))) {
      final overdue = due.isBefore(effectiveNow);
      candidates.add(
        MkTodayItem(
          key: 'record:${record.kind}:${record.entityId}',
          group: overdue ? MkTodayGroup.blockedAndDue : MkTodayGroup.upcoming,
          priority: overdue ? 85 : 65,
          title: record.title,
          detail: overdue ? 'Overdue' : '${record.type.label} is coming up',
          record: record,
          dueAt: due,
        ),
      );
    }
  }
  for (final run in snapshot.agentRuns) {
    candidates.add(
      MkTodayItem(
        key: 'run:${run.runId}',
        group: MkTodayGroup.agentOutcomes,
        priority: run.status.isFailure ? 80 : 40,
        title: '${run.persona.label}: ${run.status.label}',
        detail: run.errorMessage ?? 'Manual work remains available.',
        run: run,
      ),
    );
  }

  final deduplicated = <String, MkTodayItem>{};
  for (final item in candidates) {
    final previous = deduplicated[item.key];
    if (previous == null || item.priority > previous.priority) {
      deduplicated[item.key] = item;
    }
  }
  final items = deduplicated.values.toList(growable: false);
  items.sort((a, b) {
    final priority = b.priority.compareTo(a.priority);
    if (priority != 0) return priority;
    if (a.dueAt != null && b.dueAt != null) {
      return a.dueAt!.compareTo(b.dueAt!);
    }
    if (a.dueAt != null) return -1;
    if (b.dueAt != null) return 1;
    return a.title.compareTo(b.title);
  });
  return items;
}

DateTime? _recordDate(MkRecord record) {
  for (final key in ['deadline', 'due_at', 'scheduled_at']) {
    final value = record.data[key];
    if (value is String) {
      final parsed = DateTime.tryParse(value);
      if (parsed != null) return parsed;
    }
  }
  return null;
}

bool _isBlocked(MkRecord record) =>
    record.status == MkTaskStatus.blocked.wireName ||
    record.status == MkProjectStatus.blocked.wireName;
