import 'package:file_selector/file_selector.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../../shared/mkideas/mkideas_provider.dart';
import '../../shared/relay/nostr_models.dart';
import '../../shared/theme/theme.dart';

class MkStudioPage extends ConsumerWidget {
  const MkStudioPage({required this.onSearch, super.key});
  final VoidCallback onSearch;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final snapshot = ref.watch(mkIdeasProvider);
    return Scaffold(
      backgroundColor: context.colors.surface,
      body: SafeArea(
        child: snapshot.when(
          loading: () => const Center(child: CircularProgressIndicator()),
          error: (error, _) => Center(child: Text('$error')),
          data: (data) {
            final interviews = data.records.where((record) => record.recordType == 'interview').toList();
            final approvals = {
              for (final record in data.records.where((record) => record.recordType == 'approval'))
                '${record.data['proposal_id'] ?? ''}': record.status,
            };
            final studioProposals = data.proposals.where((proposal) => proposal.targetKind == EventKind.mkInterview || proposal.targetKind == EventKind.mkContent).toList();
            return RefreshIndicator(
              onRefresh: () => ref.refresh(mkIdeasProvider.future),
              child: ListView(
                padding: const EdgeInsets.fromLTRB(Grid.gutter, Grid.lg, Grid.gutter, 120),
                children: [
                  Row(children: [
                    Expanded(child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
                      Text('INTERVIEW TO IDEA', style: context.textTheme.labelSmall?.copyWith(color: context.colors.primary, letterSpacing: 1.6, fontWeight: FontWeight.w700)),
                      Text('Studio', style: context.textTheme.headlineLarge?.copyWith(fontFamily: 'Georgia', fontWeight: FontWeight.w700)),
                    ])),
                    IconButton(onPressed: onSearch, icon: const Icon(LucideIcons.search)),
                  ]),
                  const SizedBox(height: Grid.lg),
                  Text('INTERVIEWS', style: context.textTheme.labelSmall?.copyWith(color: context.colors.primary, letterSpacing: 1.4, fontWeight: FontWeight.w700)),
                  const SizedBox(height: Grid.sm),
                  if (interviews.isEmpty)
                    const _Panel(text: 'Create an interview from a guest in People.')
                  else
                    for (final interview in interviews)
                      Card(
                        margin: const EdgeInsets.only(bottom: Grid.sm),
                        child: Padding(
                          padding: const EdgeInsets.all(Grid.md),
                          child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
                            Text(interview.title, style: context.textTheme.titleMedium?.copyWith(fontFamily: 'Georgia', fontWeight: FontWeight.w700)),
                            Text(interview.status.replaceAll('_', ' '), style: context.textTheme.bodySmall?.copyWith(color: context.colors.onSurfaceVariant)),
                            const SizedBox(height: Grid.sm),
                            Wrap(spacing: Grid.sm, runSpacing: Grid.sm, children: [
                              OutlinedButton.icon(onPressed: () => _attachTranscript(ref, interview), icon: const Icon(LucideIcons.upload, size: 16), label: const Text('Upload transcript')),
                              OutlinedButton.icon(onPressed: () => ref.read(mkIdeasProvider.notifier).createContent(interview), icon: const Icon(LucideIcons.fileText, size: 16), label: const Text('Create content record')),
                              IconButton(
                                onPressed: () => Clipboard.setData(ClipboardData(text: '${interview.title} — buzz://mkideas?kind=${interview.kind}&id=${interview.entityId}')),
                                icon: const Icon(LucideIcons.copy),
                                tooltip: 'Copy Team reference',
                              ),
                            ]),
                          ]),
                        ),
                      ),
                  const SizedBox(height: Grid.lg),
                  Text('CLIP & CAPTION DESK', style: context.textTheme.labelSmall?.copyWith(color: context.colors.primary, letterSpacing: 1.4, fontWeight: FontWeight.w700)),
                  const SizedBox(height: Grid.sm),
                  if (studioProposals.isEmpty)
                    const _Panel(text: 'The Content / Clip Copilot’s timestamped proposals appear here. AI proposes; a partner decides.')
                  else
                    for (final proposal in studioProposals)
                      Card(
                        margin: const EdgeInsets.only(bottom: Grid.sm),
                        child: Padding(
                          padding: const EdgeInsets.all(Grid.md),
                          child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
                            Text('${proposal.agent.toUpperCase()} · PROPOSED', style: context.textTheme.labelSmall?.copyWith(color: context.colors.primary, fontWeight: FontWeight.w700)),
                            const SizedBox(height: Grid.xs),
                            Text(proposal.summary, style: context.textTheme.titleMedium?.copyWith(fontFamily: 'Georgia', fontWeight: FontWeight.w700)),
                            for (final clip in proposal.clips) Padding(
                              padding: const EdgeInsets.only(top: Grid.sm),
                              child: Text('${clip['start'] ?? ''}—${clip['end'] ?? ''}  ${clip['title'] ?? ''}\n${clip['caption'] ?? ''}', style: context.textTheme.bodySmall),
                            ),
                            const SizedBox(height: Grid.sm),
                            if (approvals[proposal.proposalId] case final decision?)
                              Text('HUMAN REVIEW · ${decision.toUpperCase()}', style: context.textTheme.labelSmall?.copyWith(color: context.colors.primary, fontWeight: FontWeight.w700))
                            else
                              Wrap(spacing: Grid.sm, children: [
                                FilledButton(onPressed: () => ref.read(mkIdeasProvider.notifier).reviewProposal(proposal, 'approved'), child: const Text('Approve')),
                                OutlinedButton(onPressed: () => ref.read(mkIdeasProvider.notifier).reviewProposal(proposal, 'rejected'), child: const Text('Reject')),
                                IconButton(
                                  onPressed: () => Clipboard.setData(ClipboardData(text: '${proposal.summary} — buzz://mkideas?kind=${proposal.targetKind}&id=${proposal.targetId}&proposal=${proposal.proposalId}')),
                                  icon: const Icon(LucideIcons.copy),
                                  tooltip: 'Copy Team reference',
                                ),
                              ]),
                          ]),
                        ),
                      ),
                ],
              ),
            );
          },
        ),
      ),
    );
  }

  Future<void> _attachTranscript(WidgetRef ref, MkRecord interview) async {
    const types = XTypeGroup(label: 'Transcript', extensions: ['txt', 'vtt', 'srt']);
    final file = await openFile(acceptedTypeGroups: [types]);
    if (file == null) return;
    final text = await file.readAsString();
    await ref.read(mkIdeasProvider.notifier).attachTranscript(interview, file.name, text);
  }
}

class _Panel extends StatelessWidget {
  const _Panel({required this.text});
  final String text;
  @override
  Widget build(BuildContext context) => Container(
    padding: const EdgeInsets.all(Grid.lg),
    decoration: BoxDecoration(border: Border.all(color: context.colors.outlineVariant), borderRadius: BorderRadius.circular(Radii.md)),
    child: Text(text, textAlign: TextAlign.center, style: context.textTheme.bodyMedium?.copyWith(color: context.colors.onSurfaceVariant)),
  );
}
