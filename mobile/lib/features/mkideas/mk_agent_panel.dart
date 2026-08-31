import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../../shared/mkideas/mkideas_provider.dart';
import '../../shared/theme/theme.dart';

/// Contextual, human-gated status for one of the five MK agent personas.
class MkAgentPanel extends StatelessWidget {
  const MkAgentPanel({
    required this.persona,
    required this.snapshot,
    this.target,
    this.onLaunch,
    super.key,
  });

  final MkAgentPersona persona;
  final MkIdeasSnapshot snapshot;
  final MkRecord? target;
  final VoidCallback? onLaunch;

  @override
  Widget build(BuildContext context) {
    final runs = snapshot.agentRuns.where(_runMatches).toList(growable: false);
    final proposals = snapshot.proposals.where(_proposalMatches).toList();
    final latestRun = runs.isEmpty ? null : runs.first;
    final latestProposal = proposals.isEmpty ? null : proposals.first;
    return Container(
      key: ValueKey('mk-agent-${persona.id}'),
      padding: const EdgeInsets.all(Grid.md),
      decoration: BoxDecoration(
        color: context.colors.surfaceContainerLow,
        border: Border.all(color: context.colors.outlineVariant),
        borderRadius: BorderRadius.circular(Radii.md),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Icon(
                LucideIcons.sparkles,
                size: 18,
                color: context.colors.primary,
              ),
              const SizedBox(width: Grid.xs),
              Expanded(
                child: Text(
                  persona.label,
                  style: context.textTheme.titleSmall?.copyWith(
                    fontFamily: 'Georgia',
                    fontWeight: FontWeight.w700,
                  ),
                ),
              ),
              _StateBadge(run: latestRun, proposal: latestProposal),
            ],
          ),
          const SizedBox(height: Grid.xs),
          Text(
            persona.purpose,
            style: context.textTheme.bodySmall?.copyWith(
              color: context.colors.onSurfaceVariant,
            ),
          ),
          if (latestRun?.errorMessage case final error?) ...[
            const SizedBox(height: Grid.xs),
            Text(error, style: context.textTheme.bodySmall),
          ],
          if (latestProposal != null) ...[
            const SizedBox(height: Grid.sm),
            Text(latestProposal.summary, style: context.textTheme.bodyMedium),
            const SizedBox(height: Grid.half),
            Text(
              latestProposal.isStale
                  ? 'STALE · A fresh run is required before review.'
                  : '${latestProposal.provenance.length} SOURCES · HUMAN REVIEW REQUIRED',
              style: context.textTheme.labelSmall?.copyWith(
                color: context.colors.primary,
                fontWeight: FontWeight.w700,
              ),
            ),
          ],
          const SizedBox(height: Grid.sm),
          Row(
            children: [
              OutlinedButton.icon(
                key: ValueKey('mk-agent-launch-${persona.id}'),
                onPressed: onLaunch,
                icon: const Icon(LucideIcons.play, size: 16),
                label: Text(latestRun == null ? 'Run agent' : 'Run again'),
              ),
              const SizedBox(width: Grid.xs),
              Expanded(
                child: Text(
                  persona.informationalOnly
                      ? 'Informational only. It cannot change state.'
                      : 'Draft only. AI cannot approve, send, or publish.',
                  style: context.textTheme.labelSmall?.copyWith(
                    color: context.colors.onSurfaceVariant,
                  ),
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }

  bool _runMatches(MkAgentRun run) =>
      run.persona == persona &&
      (target == null ||
          run.target != null && run.target!.entityId == target!.entityId);

  bool _proposalMatches(MkProposal proposal) =>
      _samePersona(proposal) &&
      (target == null ||
          proposal.targetId == target?.entityId &&
              proposal.targetKind == target?.kind);

  bool _samePersona(MkProposal proposal) {
    final candidate = proposal.personaId ?? proposal.agent;
    String normalize(String value) =>
        value.toLowerCase().replaceAll(RegExp('[^a-z0-9]'), '');
    return normalize(candidate) == normalize(persona.id) ||
        normalize(candidate) == normalize(persona.label);
  }
}

class _StateBadge extends StatelessWidget {
  const _StateBadge({required this.run, required this.proposal});

  final MkAgentRun? run;
  final MkProposal? proposal;

  @override
  Widget build(BuildContext context) {
    final label = proposal?.isStale == true
        ? 'Stale'
        : proposal != null
        ? 'Review'
        : run?.status.label ?? 'Ready';
    return Container(
      padding: const EdgeInsets.symmetric(
        horizontal: Grid.xs,
        vertical: Grid.half,
      ),
      decoration: BoxDecoration(
        border: Border.all(color: context.colors.primary),
        borderRadius: BorderRadius.circular(Radii.full),
      ),
      child: Text(
        label.toUpperCase(),
        style: context.textTheme.labelSmall?.copyWith(
          color: context.colors.primary,
          fontWeight: FontWeight.w700,
        ),
      ),
    );
  }
}
