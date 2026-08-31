import 'package:flutter/material.dart';
import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../../shared/mkideas/mkideas_provider.dart';
import '../../shared/theme/theme.dart';
import 'mk_agent_panel.dart';

class MkStudioRecordPage extends HookConsumerWidget {
  const MkStudioRecordPage({required this.record, super.key});

  final MkRecord record;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final snapshot = ref.watch(mkIdeasProvider).value ?? MkIdeasSnapshot.empty;
    final current = snapshot.headsByCoordinate[record.coordinate] ?? record;
    final historyError = useState<String?>(null);
    useEffect(() {
      Future<void>(() async {
        try {
          await ref
              .read(mkIdeasProvider.notifier)
              .loadHistoryPage(current.coordinate);
        } catch (error) {
          if (context.mounted) historyError.value = '$error';
        }
      });
      return null;
    }, [current.coordinate]);
    final proposals = snapshot.proposals.where(
      (proposal) =>
          proposal.targetKind == current.kind &&
          proposal.targetId == current.entityId,
    );
    final revisions = snapshot.revisionsFor(current.type, current.entityId);
    final transcript = current.payload is MkInterviewData
        ? (current.payload as MkInterviewData).transcript
        : null;
    final personas = current.type == MkEntityType.interview
        ? const [
            MkAgentPersona.interviewProducer,
            MkAgentPersona.contentClipCopilot,
          ]
        : const [MkAgentPersona.contentClipCopilot];
    return Scaffold(
      backgroundColor: context.colors.surface,
      appBar: AppBar(title: Text(current.type.label)),
      body: ListView(
        padding: const EdgeInsets.fromLTRB(
          Grid.gutter,
          Grid.md,
          Grid.gutter,
          Grid.xl,
        ),
        children: [
          Text(
            current.title,
            style: context.textTheme.headlineMedium?.copyWith(
              fontFamily: 'Georgia',
              fontWeight: FontWeight.w700,
            ),
          ),
          Text(
            '${current.status.replaceAll('-', ' ')} · version ${current.version}',
            style: context.textTheme.bodySmall?.copyWith(
              color: context.colors.onSurfaceVariant,
            ),
          ),
          if (transcript != null) ...[
            const SizedBox(height: Grid.lg),
            const _SectionLabel('Private transcript'),
            _TranscriptCard(descriptor: transcript),
          ],
          const SizedBox(height: Grid.lg),
          const _SectionLabel('Contextual agents'),
          const SizedBox(height: Grid.sm),
          for (final persona in personas) ...[
            MkAgentPanel(
              persona: persona,
              snapshot: snapshot,
              target: current,
              onLaunch: () => _showRunnerBoundary(context, persona),
            ),
            const SizedBox(height: Grid.sm),
          ],
          const SizedBox(height: Grid.md),
          const _SectionLabel('Drafts and clip proposals'),
          if (proposals.isEmpty)
            const _Empty('No agent draft is attached to this version.')
          else
            for (final proposal in proposals)
              _ProposalReviewCard(
                proposal: proposal,
                reviewed: snapshot.isProposalReviewed(proposal.proposalId),
                onReview: (decision) =>
                    _review(context, ref, proposal, decision),
              ),
          const SizedBox(height: Grid.lg),
          const _SectionLabel('Signed history'),
          if (historyError.value != null)
            _Empty('History sync issue: ${historyError.value}')
          else if (revisions.isEmpty)
            const _Empty('Loading signed revision history…')
          else
            for (final revision in revisions.reversed)
              ListTile(
                contentPadding: EdgeInsets.zero,
                leading: const Icon(LucideIcons.history, size: 18),
                title: Text(
                  'Version ${revision.version} · '
                  '${revision.status.replaceAll('-', ' ')}',
                ),
                subtitle: Text('Signed by ${_short(revision.author)}'),
              ),
        ],
      ),
    );
  }
}

Future<void> _review(
  BuildContext context,
  WidgetRef ref,
  MkProposal proposal,
  String decision,
) async {
  try {
    await ref.read(mkIdeasProvider.notifier).reviewProposal(proposal, decision);
  } catch (error) {
    if (!context.mounted) return;
    ScaffoldMessenger.of(
      context,
    ).showSnackBar(SnackBar(content: Text('$error')));
  }
}

void _showRunnerBoundary(BuildContext context, MkAgentPersona persona) {
  ScaffoldMessenger.of(context).showSnackBar(
    SnackBar(
      content: Text(
        '${persona.label} requires a configured runner. '
        'Manual Studio work stays available.',
      ),
    ),
  );
}

class _TranscriptCard extends StatelessWidget {
  const _TranscriptCard({required this.descriptor});
  final MkMediaDescriptor descriptor;

  @override
  Widget build(BuildContext context) => Container(
    key: const Key('mk-transcript-descriptor'),
    margin: const EdgeInsets.only(top: Grid.sm),
    padding: const EdgeInsets.all(Grid.md),
    decoration: BoxDecoration(
      color: context.colors.surfaceContainerLow,
      border: Border.all(color: context.colors.outlineVariant),
      borderRadius: BorderRadius.circular(Radii.md),
    ),
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(descriptor.filename, style: context.textTheme.titleSmall),
        const SizedBox(height: Grid.half),
        Text(
          '${descriptor.mimeType} · ${_fileSize(descriptor.sizeBytes)} · '
          'version ${descriptor.version}',
          style: context.textTheme.bodySmall?.copyWith(
            color: context.colors.onSurfaceVariant,
          ),
        ),
        Text(
          'SHA-256 ${_short(descriptor.sha256)} · private media',
          style: context.textTheme.labelSmall?.copyWith(
            color: context.colors.onSurfaceVariant,
          ),
        ),
      ],
    ),
  );
}

class _ProposalReviewCard extends StatelessWidget {
  const _ProposalReviewCard({
    required this.proposal,
    required this.reviewed,
    required this.onReview,
  });

  final MkProposal proposal;
  final bool reviewed;
  final void Function(String) onReview;

  @override
  Widget build(BuildContext context) => Container(
    margin: const EdgeInsets.only(top: Grid.sm),
    padding: const EdgeInsets.all(Grid.md),
    decoration: BoxDecoration(
      border: Border.all(color: context.colors.outlineVariant),
      borderRadius: BorderRadius.circular(Radii.md),
    ),
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(proposal.summary, style: context.textTheme.titleSmall),
        for (final clip in proposal.clips)
          Padding(
            padding: const EdgeInsets.only(top: Grid.sm),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  '${_timestamp(clip['start_ms'])}–${_timestamp(clip['end_ms'])} '
                  '${clip['title'] ?? 'Clip'}',
                  style: context.textTheme.labelMedium,
                ),
                if (clip['caption'] case final String caption) Text(caption),
              ],
            ),
          ),
        const SizedBox(height: Grid.sm),
        Text(
          reviewed
              ? 'HUMAN REVIEW COMPLETE'
              : proposal.isStale
              ? 'STALE · FRESH RUN REQUIRED'
              : 'DRAFT · HUMAN DECISION REQUIRED',
          style: context.textTheme.labelSmall?.copyWith(
            color: context.colors.primary,
            fontWeight: FontWeight.w700,
          ),
        ),
        if (!reviewed && proposal.isApprovable) ...[
          const SizedBox(height: Grid.sm),
          Wrap(
            spacing: Grid.sm,
            children: [
              FilledButton(
                onPressed: () => onReview('approved'),
                child: const Text('Approve'),
              ),
              OutlinedButton(
                onPressed: () => onReview('rejected'),
                child: const Text('Reject'),
              ),
            ],
          ),
        ],
      ],
    ),
  );
}

String _timestamp(Object? value) {
  final milliseconds = value is int ? value : 0;
  final totalSeconds = milliseconds ~/ 1000;
  final minutes = totalSeconds ~/ 60;
  final seconds = totalSeconds % 60;
  return '$minutes:${seconds.toString().padLeft(2, '0')}';
}

String _fileSize(int value) => value < 1024 * 1024
    ? '${(value / 1024).toStringAsFixed(0)} KB'
    : '${(value / 1024 / 1024).toStringAsFixed(1)} MB';

String _short(String value) => value.length <= 16
    ? value
    : '${value.substring(0, 8)}…${value.substring(value.length - 6)}';

class _SectionLabel extends StatelessWidget {
  const _SectionLabel(this.label);
  final String label;

  @override
  Widget build(BuildContext context) => Text(
    label.toUpperCase(),
    style: context.textTheme.labelSmall?.copyWith(
      color: context.colors.primary,
      fontWeight: FontWeight.w700,
      letterSpacing: 1.4,
    ),
  );
}

class _Empty extends StatelessWidget {
  const _Empty(this.message);
  final String message;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.symmetric(vertical: Grid.sm),
    child: Text(
      message,
      style: context.textTheme.bodySmall?.copyWith(
        color: context.colors.onSurfaceVariant,
      ),
    ),
  );
}
