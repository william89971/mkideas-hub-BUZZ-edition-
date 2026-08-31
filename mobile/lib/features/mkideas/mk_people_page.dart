import 'package:flutter/material.dart';
import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:flutter/services.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../../shared/deeplink/deep_link.dart';
import '../../shared/mkideas/mkideas_provider.dart';
import '../../shared/relay/relay_provider.dart';
import '../../shared/theme/theme.dart';
import 'mk_person_detail_page.dart';

class MkPeoplePage extends HookConsumerWidget {
  const MkPeoplePage({required this.onSearch, super.key});
  final VoidCallback onSearch;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final snapshot = ref.watch(mkIdeasProvider);
    final community = mkCommunityHost(ref.watch(relayConfigProvider).wsUrl);
    final query = useState('');
    return Scaffold(
      backgroundColor: context.colors.surface,
      body: SafeArea(
        child: Padding(
          padding: const EdgeInsets.fromLTRB(
            Grid.gutter,
            Grid.lg,
            Grid.gutter,
            0,
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          'GUEST PIPELINE',
                          style: context.textTheme.labelSmall?.copyWith(
                            color: context.colors.primary,
                            letterSpacing: 1.6,
                            fontWeight: FontWeight.w700,
                          ),
                        ),
                        Text(
                          'People',
                          style: context.textTheme.headlineLarge?.copyWith(
                            fontFamily: 'Georgia',
                            fontWeight: FontWeight.w700,
                          ),
                        ),
                      ],
                    ),
                  ),
                  IconButton(
                    onPressed: onSearch,
                    icon: const Icon(LucideIcons.search),
                  ),
                ],
              ),
              const SizedBox(height: Grid.md),
              TextField(
                onChanged: (value) => query.value = value,
                decoration: const InputDecoration(
                  prefixIcon: Icon(LucideIcons.search),
                  hintText: 'Filter people',
                ),
              ),
              const SizedBox(height: Grid.sm),
              Expanded(
                child: snapshot.when(
                  loading: () =>
                      const Center(child: CircularProgressIndicator()),
                  error: (error, _) => Center(child: Text('$error')),
                  data: (data) {
                    final people = data
                        .recordsOfType(MkEntityType.person)
                        .where(
                          (record) => record.title.toLowerCase().contains(
                            query.value.toLowerCase(),
                          ),
                        )
                        .toList();
                    if (people.isEmpty) {
                      return const Center(
                        child: Text('No guests match this view.'),
                      );
                    }
                    return RefreshIndicator(
                      onRefresh: () => ref.refresh(mkIdeasProvider.future),
                      child: ListView.separated(
                        padding: const EdgeInsets.only(bottom: 120),
                        itemCount: people.length,
                        separatorBuilder: (_, _) => const Divider(height: 1),
                        itemBuilder: (context, index) {
                          final person = people[index];
                          final research = data.proposals
                              .where(
                                (proposal) =>
                                    proposal.targetKind == person.kind &&
                                    proposal.targetId == person.entityId,
                              )
                              .toList();
                          return Card(
                            margin: const EdgeInsets.symmetric(
                              vertical: Grid.xs,
                            ),
                            child: Column(
                              crossAxisAlignment: CrossAxisAlignment.stretch,
                              children: [
                                ListTile(
                                  onTap: () => Navigator.of(context).push(
                                    MaterialPageRoute<void>(
                                      builder: (_) =>
                                          MkPersonDetailPage(person: person),
                                    ),
                                  ),
                                  contentPadding: const EdgeInsets.symmetric(
                                    horizontal: Grid.md,
                                    vertical: Grid.xs,
                                  ),
                                  title: Text(
                                    person.title,
                                    style: context.textTheme.titleMedium
                                        ?.copyWith(
                                          fontFamily: 'Georgia',
                                          fontWeight: FontWeight.w700,
                                        ),
                                  ),
                                  subtitle: Text(
                                    '${person.data['organization'] ?? 'Independent'} · '
                                    '${person.status.replaceAll('_', ' ')}'
                                    '${person.payload is MkPersonData && (person.payload as MkPersonData).doNotContact ? ' · DNC' : ''}',
                                  ),
                                  trailing: Row(
                                    mainAxisSize: MainAxisSize.min,
                                    children: [
                                      IconButton(
                                        onPressed: () {
                                          final link = buildMkIdeasEntityLink(
                                            community: community,
                                            kind: person.kind,
                                            entityId: person.entityId,
                                          );
                                          Clipboard.setData(
                                            ClipboardData(
                                              text: '${person.title} — $link',
                                            ),
                                          );
                                        },
                                        icon: const Icon(LucideIcons.copy),
                                        tooltip: 'Copy Team reference',
                                      ),
                                      IconButton(
                                        onPressed: () => ref
                                            .read(mkIdeasProvider.notifier)
                                            .createInterview(person),
                                        icon: const Icon(LucideIcons.video),
                                        tooltip: 'Create interview',
                                      ),
                                    ],
                                  ),
                                ),
                                for (final proposal in research)
                                  Container(
                                    margin: const EdgeInsets.fromLTRB(
                                      Grid.md,
                                      0,
                                      Grid.md,
                                      Grid.md,
                                    ),
                                    padding: const EdgeInsets.all(Grid.md),
                                    decoration: BoxDecoration(
                                      border: Border(
                                        left: BorderSide(
                                          color: context.colors.primary,
                                          width: 2,
                                        ),
                                      ),
                                      color: context.colors.surfaceContainerLow,
                                    ),
                                    child: Column(
                                      crossAxisAlignment:
                                          CrossAxisAlignment.start,
                                      children: [
                                        Text(
                                          'ATTACHED RESEARCH · ${proposal.agent.toUpperCase()}',
                                          style: context.textTheme.labelSmall
                                              ?.copyWith(
                                                color: context.colors.primary,
                                                fontWeight: FontWeight.w700,
                                              ),
                                        ),
                                        const SizedBox(height: Grid.xs),
                                        Text(proposal.summary),
                                        const SizedBox(height: Grid.xs),
                                        Text(
                                          '${proposal.provenance.length} provenance sources · '
                                          'proposal v${proposal.proposalVersion}'
                                          '${proposal.isStale ? ' · stale' : ' · human review required'}',
                                          style: context.textTheme.bodySmall
                                              ?.copyWith(
                                                color: context
                                                    .colors
                                                    .onSurfaceVariant,
                                              ),
                                        ),
                                      ],
                                    ),
                                  ),
                              ],
                            ),
                          );
                        },
                      ),
                    );
                  },
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
