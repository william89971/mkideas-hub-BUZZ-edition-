import 'package:flutter/material.dart';
import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../../shared/mkideas/mkideas_provider.dart';
import '../../shared/theme/theme.dart';
import 'mk_agent_panel.dart';

class MkPersonDetailPage extends HookConsumerWidget {
  const MkPersonDetailPage({required this.person, super.key});

  final MkRecord person;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final snapshot = ref.watch(mkIdeasProvider).value ?? MkIdeasSnapshot.empty;
    final current = snapshot.headsByCoordinate[person.coordinate] ?? person;
    final payload = current.payload as MkPersonData;
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
    final interviews = snapshot
        .recordsOfType(MkEntityType.interview)
        .where(
          (record) =>
              record.payload is MkInterviewData &&
              (record.payload as MkInterviewData).guestId == current.entityId,
        );
    final proposals = snapshot.proposals.where(
      (proposal) =>
          proposal.targetKind == current.kind &&
          proposal.targetId == current.entityId,
    );
    final revisions = snapshot.revisionsFor(current.type, current.entityId);
    return Scaffold(
      backgroundColor: context.colors.surface,
      appBar: AppBar(title: const Text('Guest profile')),
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
          const SizedBox(height: Grid.xs),
          Text(
            [
              payload.titleOrRole,
              payload.organization,
            ].whereType<String>().join(' · '),
            style: context.textTheme.bodyMedium?.copyWith(
              color: context.colors.onSurfaceVariant,
            ),
          ),
          if (payload.doNotContact) ...[
            const SizedBox(height: Grid.sm),
            const _DncNotice(),
          ],
          const SizedBox(height: Grid.lg),
          const _SectionLabel('Relationship'),
          _DetailRow('Status', current.status.replaceAll('-', ' ')),
          if (payload.owner != null) _DetailRow('Owner', payload.owner!),
          if (payload.email != null) _DetailRow('Email', payload.email!),
          if (payload.whyNow != null) _DetailRow('Why now', payload.whyNow!),
          if (payload.topics.isNotEmpty)
            _DetailRow('Topics', payload.topics.join(', ')),
          const SizedBox(height: Grid.lg),
          const _SectionLabel('Contextual agents'),
          const SizedBox(height: Grid.sm),
          for (final persona in const [
            MkAgentPersona.guestResearcher,
            MkAgentPersona.outreachDrafter,
            MkAgentPersona.interviewProducer,
          ]) ...[
            MkAgentPanel(
              persona: persona,
              snapshot: snapshot,
              target: current,
              onLaunch: () => _showRunnerBoundary(context, persona),
            ),
            const SizedBox(height: Grid.sm),
          ],
          const SizedBox(height: Grid.md),
          const _SectionLabel('Research and drafts'),
          if (proposals.isEmpty)
            const _Empty('No sourced agent drafts yet.')
          else
            for (final proposal in proposals)
              ListTile(
                contentPadding: EdgeInsets.zero,
                leading: const Icon(LucideIcons.fileSearch),
                title: Text(proposal.summary),
                subtitle: Text(
                  proposal.isStale
                      ? 'Stale · fresh run required'
                      : '${proposal.provenance.length} sources · human review required',
                ),
              ),
          const SizedBox(height: Grid.lg),
          const _SectionLabel('Interviews'),
          if (interviews.isEmpty)
            const _Empty('No interview is linked yet.')
          else
            for (final interview in interviews)
              ListTile(
                contentPadding: EdgeInsets.zero,
                leading: const Icon(LucideIcons.mic2),
                title: Text(interview.title),
                subtitle: Text(interview.status.replaceAll('-', ' ')),
              ),
          const SizedBox(height: Grid.lg),
          const _SectionLabel('Relationship timeline'),
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

void _showRunnerBoundary(BuildContext context, MkAgentPersona persona) {
  ScaffoldMessenger.of(context).showSnackBar(
    SnackBar(
      content: Text(
        '${persona.label} is ready for a configured runner. '
        'This mobile build will not simulate or silently publish a run.',
      ),
    ),
  );
}

String _short(String value) => value.length <= 12
    ? value
    : '${value.substring(0, 6)}…${value.substring(value.length - 4)}';

class _DncNotice extends StatelessWidget {
  const _DncNotice();

  @override
  Widget build(BuildContext context) => Container(
    padding: const EdgeInsets.all(Grid.sm),
    decoration: BoxDecoration(
      color: context.colors.errorContainer,
      borderRadius: BorderRadius.circular(Radii.md),
    ),
    child: Row(
      children: [
        Icon(LucideIcons.shieldAlert, color: context.colors.onErrorContainer),
        const SizedBox(width: Grid.sm),
        Expanded(
          child: Text(
            'DO NOT CONTACT · Outreach drafting is blocked. Only the owner can clear this with an audited reason.',
            style: context.textTheme.labelSmall?.copyWith(
              color: context.colors.onErrorContainer,
              fontWeight: FontWeight.w700,
            ),
          ),
        ),
      ],
    ),
  );
}

class _DetailRow extends StatelessWidget {
  const _DetailRow(this.label, this.value);
  final String label;
  final String value;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.symmetric(vertical: Grid.xs),
    child: Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        SizedBox(
          width: 92,
          child: Text(
            label,
            style: context.textTheme.labelSmall?.copyWith(
              color: context.colors.onSurfaceVariant,
            ),
          ),
        ),
        Expanded(child: Text(value)),
      ],
    ),
  );
}

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
