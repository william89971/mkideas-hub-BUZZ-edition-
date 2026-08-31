import 'package:flutter/material.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../../shared/mkideas/mkideas_provider.dart';
import '../../shared/relay/relay.dart';
import '../../shared/theme/theme.dart';

/// Presents actionable MK relay synchronization failures above every area.
class MkSyncHealthBanner extends ConsumerWidget {
  const MkSyncHealthBanner({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final snapshot = ref.watch(mkIdeasProvider);
    final session = ref.watch(relaySessionProvider);
    final issue = snapshot.when(
      loading: () => 'Synchronizing MK Ideas',
      error: (error, _) => 'MK Ideas unavailable',
      data: (data) {
        if (data.syncIssue != null) return 'Live sync paused';
        return switch (session.status) {
          SessionStatus.disconnected => 'MK Ideas is offline',
          SessionStatus.connecting => 'Connecting MK Ideas',
          SessionStatus.reconnecting => 'Reconnecting MK Ideas',
          SessionStatus.connected => null,
        };
      },
    );
    if (issue == null) return const SizedBox.shrink();

    return SafeArea(
      minimum: const EdgeInsets.fromLTRB(Grid.gutter, Grid.xxs, Grid.gutter, 0),
      child: Align(
        alignment: Alignment.topCenter,
        child: Material(
          key: const Key('mk-sync-health-banner'),
          color: context.colors.errorContainer,
          borderRadius: BorderRadius.circular(Radii.full),
          child: InkWell(
            borderRadius: BorderRadius.circular(Radii.full),
            onTap: () => ref.invalidate(mkIdeasProvider),
            child: Padding(
              padding: const EdgeInsets.symmetric(
                horizontal: Grid.xs,
                vertical: Grid.xxs,
              ),
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Icon(
                    LucideIcons.cloudOff,
                    size: 16,
                    color: context.colors.onErrorContainer,
                  ),
                  const SizedBox(width: Grid.xxs),
                  Text(
                    '$issue · Tap to retry',
                    style: context.textTheme.labelSmall?.copyWith(
                      color: context.colors.onErrorContainer,
                      fontWeight: FontWeight.w600,
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}
