part of '../search_page.dart';

class _MkIdeasSection extends StatelessWidget {
  const _MkIdeasSection({
    required this.results,
    required this.onResultSelected,
    required this.onOpen,
  });

  final List<MkIdeasSearchResult> results;
  final VoidCallback onResultSelected;
  final ValueChanged<MkIdeasSearchResult>? onOpen;

  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.start,
    children: [
      const _SectionLabel(label: 'MK Ideas'),
      for (final result in results)
        ListTile(
          key: ValueKey('search-mkideas-row-${result.sourceEventId}'),
          contentPadding: const EdgeInsets.symmetric(horizontal: Grid.gutter),
          leading: Container(
            padding: const EdgeInsets.all(Grid.xxs),
            decoration: BoxDecoration(
              color: context.colors.primaryContainer,
              borderRadius: BorderRadius.circular(Radii.md),
            ),
            child: Icon(
              _mkSearchIcon(result.entityType),
              size: 20,
              color: context.colors.onPrimaryContainer,
            ),
          ),
          title: Text(result.title, style: contentListTitleTextStyle),
          subtitle: Text(
            '${result.entityType.label} · ${result.summary}',
            maxLines: 2,
            overflow: TextOverflow.ellipsis,
            style: contentListBodyTextStyle.copyWith(
              color: context.colors.onSurfaceVariant,
            ),
          ),
          trailing: Icon(
            LucideIcons.chevronRight,
            size: 18,
            color: context.colors.onSurfaceVariant,
          ),
          onTap: onOpen == null
              ? null
              : () {
                  onResultSelected();
                  onOpen!(result);
                },
        ),
    ],
  );
}

IconData _mkSearchIcon(MkEntityType type) => switch (type.productArea) {
  MkProductArea.today => LucideIcons.circleCheckBig,
  MkProductArea.work => LucideIcons.listChecks,
  MkProductArea.people => LucideIcons.userRound,
  MkProductArea.studio => LucideIcons.video,
};

class _RecentSearches extends StatelessWidget {
  const _RecentSearches({
    required this.searches,
    required this.onSelected,
    required this.onClear,
  });

  final List<String> searches;
  final ValueChanged<String> onSelected;
  final VoidCallback onClear;

  @override
  Widget build(BuildContext context) => ListView(
    key: const Key('recent-searches-list'),
    padding: EdgeInsets.only(
      bottom:
          Grid.xl +
          MediaQuery.paddingOf(context).bottom +
          MediaQuery.viewInsetsOf(context).bottom,
    ),
    children: [
      Padding(
        padding: const EdgeInsets.fromLTRB(
          Grid.gutter,
          Grid.xs,
          Grid.xxs,
          Grid.half,
        ),
        child: Row(
          children: [
            Expanded(
              child: Text(
                'Recent searches',
                key: const Key('recent-searches-heading'),
                style: activityContextTextStyle.copyWith(
                  color: context.colors.onSurfaceVariant,
                ),
              ),
            ),
            TextButton(
              key: const Key('clear-recent-searches'),
              onPressed: onClear,
              child: Text(
                'Clear',
                style: activityContextTextStyle.copyWith(
                  color: context.colors.primary,
                ),
              ),
            ),
          ],
        ),
      ),
      for (var index = 0; index < searches.length; index++)
        InkWell(
          key: ValueKey('recent-search-$index'),
          onTap: () => onSelected(searches[index]),
          child: ConstrainedBox(
            constraints: const BoxConstraints(minHeight: Grid.xl),
            child: Padding(
              padding: const EdgeInsets.symmetric(
                horizontal: Grid.gutter,
                vertical: Grid.twelve,
              ),
              child: Row(
                children: [
                  Icon(
                    LucideIcons.clock,
                    size: 18,
                    color: context.colors.onSurfaceVariant,
                  ),
                  const SizedBox(width: Grid.twelve),
                  Expanded(
                    child: Text(
                      searches[index],
                      style: contentListTitleTextStyle.copyWith(
                        color: context.colors.onSurface,
                      ),
                    ),
                  ),
                  Icon(
                    LucideIcons.chevronRight,
                    size: 16,
                    color: context.colors.onSurfaceVariant,
                  ),
                ],
              ),
            ),
          ),
        ),
    ],
  );
}
