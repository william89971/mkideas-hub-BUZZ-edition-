import 'package:flutter/material.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../../shared/mkideas/mkideas_provider.dart';
import '../../shared/theme/theme.dart';

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
          data: (data) {
            final approvals = data.records
                .where((record) => record.recordType == 'approval')
                .map((record) => record.data['proposal_id'])
                .toSet();
            final pending = data.proposals
                .where((proposal) => !approvals.contains(proposal.proposalId))
                .toList();
            final people = data.records
                .where((record) => record.recordType == 'person')
                .toList();
            return RefreshIndicator(
              onRefresh: () => ref.refresh(mkIdeasProvider.future),
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
                        child: _Metric(value: '${pending.length}', label: 'Reviews'),
                      ),
                      const SizedBox(width: Grid.sm),
                      Expanded(
                        child: _Metric(value: '${people.length}', label: 'Guests'),
                      ),
                    ],
                  ),
                  const SizedBox(height: Grid.xl),
                  Text('NEEDS YOUR JUDGMENT', style: context.textTheme.labelSmall?.copyWith(color: context.colors.primary, letterSpacing: 1.4, fontWeight: FontWeight.w700)),
                  const SizedBox(height: Grid.sm),
                  if (pending.isEmpty)
                    const _Message(text: 'No agent proposals are waiting for review.')
                  else
                    for (final proposal in pending)
                      _ProposalCard(proposal: proposal),
                  const SizedBox(height: Grid.xl),
                  Text('NEXT MOVES', style: context.textTheme.labelSmall?.copyWith(color: context.colors.primary, letterSpacing: 1.4, fontWeight: FontWeight.w700)),
                  const SizedBox(height: Grid.sm),
                  if (people.isEmpty)
                    const _Message(text: 'Add the first guest in People.')
                  else
                    for (final person in people.take(5))
                      ListTile(
                        contentPadding: EdgeInsets.zero,
                        leading: Icon(LucideIcons.clock3, color: context.colors.primary, size: 18),
                        title: Text(person.title, style: context.textTheme.titleSmall),
                        subtitle: Text(person.status.replaceAll('_', ' ')),
                      ),
                ],
              ),
            );
          },
        ),
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
            Text('COMMAND DESK', style: context.textTheme.labelSmall?.copyWith(color: context.colors.primary, letterSpacing: 1.6, fontWeight: FontWeight.w700)),
            const SizedBox(height: Grid.xs),
            Text('Today', style: context.textTheme.headlineLarge?.copyWith(fontFamily: 'Georgia', fontWeight: FontWeight.w700)),
            Text('The work that needs a human eye.', style: context.textTheme.bodyMedium?.copyWith(color: context.colors.onSurfaceVariant)),
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
    decoration: BoxDecoration(border: Border.all(color: context.colors.outlineVariant), borderRadius: BorderRadius.circular(Radii.md)),
    child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
      Text(value, style: context.textTheme.headlineMedium?.copyWith(fontFamily: 'Georgia', fontWeight: FontWeight.w700)),
      Text(label, style: context.textTheme.labelMedium),
    ]),
  );
}

class _ProposalCard extends ConsumerWidget {
  const _ProposalCard({required this.proposal});
  final MkProposal proposal;
  @override
  Widget build(BuildContext context, WidgetRef ref) => Container(
    margin: const EdgeInsets.only(bottom: Grid.sm),
    padding: const EdgeInsets.all(Grid.md),
    decoration: BoxDecoration(color: context.colors.surfaceContainerLow, border: Border.all(color: context.colors.outlineVariant), borderRadius: BorderRadius.circular(Radii.md)),
    child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
      Text('${proposal.agent.toUpperCase()} · PROPOSED', style: context.textTheme.labelSmall?.copyWith(color: context.colors.primary, fontWeight: FontWeight.w700)),
      const SizedBox(height: Grid.xs),
      Text(proposal.summary, style: context.textTheme.titleMedium?.copyWith(fontFamily: 'Georgia', fontWeight: FontWeight.w700)),
      const SizedBox(height: Grid.sm),
      Wrap(spacing: Grid.sm, children: [
        FilledButton.icon(onPressed: () => ref.read(mkIdeasProvider.notifier).reviewProposal(proposal, 'approved'), icon: const Icon(LucideIcons.check, size: 16), label: const Text('Approve')),
        OutlinedButton.icon(onPressed: () => ref.read(mkIdeasProvider.notifier).reviewProposal(proposal, 'rejected'), icon: const Icon(LucideIcons.x, size: 16), label: const Text('Reject')),
      ]),
    ]),
  );
}

class _Message extends StatelessWidget {
  const _Message({required this.text});
  final String text;
  @override
  Widget build(BuildContext context) => Container(
    padding: const EdgeInsets.all(Grid.lg),
    decoration: BoxDecoration(border: Border.all(color: context.colors.outlineVariant), borderRadius: BorderRadius.circular(Radii.md)),
    child: Text(text, textAlign: TextAlign.center, style: context.textTheme.bodyMedium?.copyWith(color: context.colors.onSurfaceVariant)),
  );
}
