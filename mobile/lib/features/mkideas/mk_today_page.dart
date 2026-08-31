import 'package:flutter/material.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../../shared/mkideas/mkideas_provider.dart';
import '../../shared/theme/theme.dart';
import 'mk_agent_panel.dart';
import 'mk_today_model.dart';

class MkTodayPage extends ConsumerWidget {
  const MkTodayPage({required this.onSearch, super.key});

  final VoidCallback onSearch;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final snapshot = ref.watch(mkIdeasProvider);
    return Scaffold(
      backgroundColor: context.colors.surface,
      body: SafeArea(
        child: snapshot.when(
          loading: () => const Center(child: CircularProgressIndicator()),
          error: (error, _) => _Message(text: '$error'),
          data: (data) => _TodayBody(
            snapshot: data,
            onSearch: onSearch,
            onRefresh: () => ref.refresh(mkIdeasProvider.future),
            onReview: (proposal, decision) =>
                _review(context, ref, proposal, decision),
          ),
        ),
      ),
    );
  }

  Future<void> _review(
    BuildContext context,
    WidgetRef ref,
    MkProposal proposal,
    String decision,
  ) async {
    try {
      await ref
          .read(mkIdeasProvider.notifier)
          .reviewProposal(proposal, decision);
    } catch (error) {
      if (!context.mounted) return;
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text('$error')));
    }
  }
}

class _TodayBody extends StatelessWidget {
  const _TodayBody({
    required this.snapshot,
    required this.onSearch,
    required this.onRefresh,
    required this.onReview,
  });

  final MkIdeasSnapshot snapshot;
  final VoidCallback onSearch;
  final Future<void> Function() onRefresh;
  final void Function(MkProposal, String) onReview;

  @override
  Widget build(BuildContext context) {
    final items = buildMkTodayItems(snapshot);
    final briefing = snapshot.summaries.isEmpty
        ? null
        : snapshot.summaries.first;
    return RefreshIndicator(
      onRefresh: onRefresh,
      child: ListView(
        padding: const EdgeInsets.fromLTRB(
          Grid.gutter,
          Grid.lg,
          Grid.gutter,
          120,
        ),
        children: [
          _Header(onSearch: onSearch),
          const SizedBox(height: Grid.lg),
          Row(
            children: [
              Expanded(
                child: _Metric(
                  value: '${snapshot.pendingProposals.length}',
                  label: 'Reviews',
                ),
              ),
              const SizedBox(width: Grid.sm),
              Expanded(
                child: _Metric(value: '${items.length}', label: 'Actionable'),
              ),
            ],
          ),
          if (briefing != null) ...[
            const SizedBox(height: Grid.lg),
            _BriefingCard(summary: briefing),
          ],
          const SizedBox(height: Grid.lg),
          MkAgentPanel(
            persona: MkAgentPersona.operationsBriefingAssistant,
            snapshot: snapshot,
            onLaunch: () => ScaffoldMessenger.of(context).showSnackBar(
              const SnackBar(
                content: Text(
                  'Operations Briefing Assistant requires a configured runner. '
                  'Today remains fully usable without it.',
                ),
              ),
            ),
          ),
          for (final group in MkTodayGroup.values) ...[
            const SizedBox(height: Grid.xl),
            _SectionLabel(text: group.label),
            const SizedBox(height: Grid.sm),
            if (items.where((item) => item.group == group).isEmpty)
              _Message(text: _emptyText(group))
            else
              for (final item in items.where((item) => item.group == group))
                _TodayCard(item: item, onReview: onReview),
          ],
        ],
      ),
    );
  }
}

class _TodayCard extends StatelessWidget {
  const _TodayCard({required this.item, required this.onReview});

  final MkTodayItem item;
  final void Function(MkProposal, String) onReview;

  @override
  Widget build(BuildContext context) => Container(
    key: ValueKey('mk-today-${item.key}'),
    margin: const EdgeInsets.only(bottom: Grid.sm),
    padding: const EdgeInsets.all(Grid.md),
    decoration: BoxDecoration(
      color: context.colors.surfaceContainerLow,
      border: Border.all(color: context.colors.outlineVariant),
      borderRadius: BorderRadius.circular(Radii.md),
    ),
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          item.title,
          style: context.textTheme.titleMedium?.copyWith(
            fontFamily: 'Georgia',
            fontWeight: FontWeight.w700,
          ),
        ),
        const SizedBox(height: Grid.half),
        Text(
          item.detail,
          style: context.textTheme.bodySmall?.copyWith(
            color: context.colors.onSurfaceVariant,
          ),
        ),
        if (item.proposal != null && item.proposal!.isApprovable) ...[
          const SizedBox(height: Grid.sm),
          Wrap(
            spacing: Grid.sm,
            children: [
              FilledButton.icon(
                onPressed: () => onReview(item.proposal!, 'approved'),
                icon: const Icon(LucideIcons.check, size: 16),
                label: const Text('Approve'),
              ),
              OutlinedButton.icon(
                onPressed: () => onReview(item.proposal!, 'rejected'),
                icon: const Icon(LucideIcons.x, size: 16),
                label: const Text('Reject'),
              ),
            ],
          ),
        ],
      ],
    ),
  );
}

class _BriefingCard extends StatelessWidget {
  const _BriefingCard({required this.summary});

  final MkGeneratedSummary summary;

  @override
  Widget build(BuildContext context) {
    final recommendations = summary.items('recommendations');
    return Container(
      padding: const EdgeInsets.all(Grid.md),
      decoration: BoxDecoration(
        border: Border(
          left: BorderSide(color: context.colors.primary, width: 3),
        ),
        color: context.colors.surfaceContainerLow,
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const _SectionLabel(text: 'Operations briefing · informational'),
          const SizedBox(height: Grid.xs),
          for (final item in recommendations) Text('• $item'),
          if (recommendations.isEmpty)
            const Text('The latest sourced operational summary is ready.'),
          const SizedBox(height: Grid.xs),
          Text(
            'AI cannot approve or change protected work.',
            style: context.textTheme.labelSmall?.copyWith(
              color: context.colors.onSurfaceVariant,
            ),
          ),
        ],
      ),
    );
  }
}

class _Header extends StatelessWidget {
  const _Header({required this.onSearch});
  final VoidCallback onSearch;

  @override
  Widget build(BuildContext context) => Row(
    crossAxisAlignment: CrossAxisAlignment.start,
    children: [
      Expanded(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const _SectionLabel(text: 'Command desk'),
            const SizedBox(height: Grid.xs),
            Text(
              'Today',
              style: context.textTheme.headlineLarge?.copyWith(
                fontFamily: 'Georgia',
                fontWeight: FontWeight.w700,
              ),
            ),
            Text(
              'Urgent, assigned, blocked, and human-gated work.',
              style: context.textTheme.bodyMedium?.copyWith(
                color: context.colors.onSurfaceVariant,
              ),
            ),
          ],
        ),
      ),
      IconButton(onPressed: onSearch, icon: const Icon(LucideIcons.search)),
    ],
  );
}

class _Metric extends StatelessWidget {
  const _Metric({required this.value, required this.label});
  final String value;
  final String label;

  @override
  Widget build(BuildContext context) => Container(
    padding: const EdgeInsets.all(Grid.md),
    decoration: BoxDecoration(
      border: Border.all(color: context.colors.outlineVariant),
      borderRadius: BorderRadius.circular(Radii.md),
    ),
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          value,
          style: context.textTheme.headlineMedium?.copyWith(
            fontFamily: 'Georgia',
            fontWeight: FontWeight.w700,
          ),
        ),
        Text(label, style: context.textTheme.labelMedium),
      ],
    ),
  );
}

class _SectionLabel extends StatelessWidget {
  const _SectionLabel({required this.text});
  final String text;

  @override
  Widget build(BuildContext context) => Text(
    text.toUpperCase(),
    style: context.textTheme.labelSmall?.copyWith(
      color: context.colors.primary,
      letterSpacing: 1.4,
      fontWeight: FontWeight.w700,
    ),
  );
}

class _Message extends StatelessWidget {
  const _Message({required this.text});
  final String text;

  @override
  Widget build(BuildContext context) => Container(
    padding: const EdgeInsets.all(Grid.lg),
    decoration: BoxDecoration(
      border: Border.all(color: context.colors.outlineVariant),
      borderRadius: BorderRadius.circular(Radii.md),
    ),
    child: Text(
      text,
      textAlign: TextAlign.center,
      style: context.textTheme.bodyMedium?.copyWith(
        color: context.colors.onSurfaceVariant,
      ),
    ),
  );
}

String _emptyText(MkTodayGroup group) => switch (group) {
  MkTodayGroup.judgment => 'No human decisions are waiting.',
  MkTodayGroup.blockedAndDue => 'Nothing blocked or overdue.',
  MkTodayGroup.upcoming => 'Nothing due in the next three days.',
  MkTodayGroup.agentOutcomes => 'No recent agent outcomes.',
};
