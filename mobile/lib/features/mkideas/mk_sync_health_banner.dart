import 'package:flutter/material.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../../shared/mkideas/mkideas_offline_store.dart';
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
    final data = switch (snapshot) {
      AsyncData(:final value) => value,
      _ => null,
    };
    final conflicts =
        data?.outbox.where((entry) => entry.conflicted).length ?? 0;
    final queued = (data?.outbox.length ?? 0) - conflicts;
    final issue = switch ((conflicts, queued)) {
      (> 0, _) =>
        '$conflicts local change${conflicts == 1 ? '' : 's'} need review',
      (_, > 0) =>
        '$queued local change${queued == 1 ? '' : 's'} waiting to sync',
      _ => snapshot.when(
        loading: () => 'Synchronizing MK Ideas',
        error: (error, _) => 'MK Ideas unavailable',
        data: (value) {
          if (value.syncIssue != null) return 'Live sync paused';
          return switch (session.status) {
            SessionStatus.disconnected => 'MK Ideas is offline',
            SessionStatus.connecting => 'Connecting MK Ideas',
            SessionStatus.reconnecting => 'Reconnecting MK Ideas',
            SessionStatus.connected => null,
          };
        },
      ),
    };
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
            onTap: () => _handleTap(context, ref, data?.outbox ?? const []),
            child: Padding(
              padding: const EdgeInsets.symmetric(
                horizontal: Grid.xs,
                vertical: Grid.xxs,
              ),
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Icon(
                    conflicts > 0
                        ? LucideIcons.triangleAlert
                        : LucideIcons.cloudOff,
                    size: 16,
                    color: context.colors.onErrorContainer,
                  ),
                  const SizedBox(width: Grid.xxs),
                  Flexible(
                    child: Text(
                      '$issue · ${conflicts > 0 ? 'Review' : 'Tap to retry'}',
                      overflow: TextOverflow.ellipsis,
                      style: context.textTheme.labelSmall?.copyWith(
                        color: context.colors.onErrorContainer,
                        fontWeight: FontWeight.w600,
                      ),
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

  Future<void> _handleTap(
    BuildContext context,
    WidgetRef ref,
    List<MkOutboxEntry> outbox,
  ) async {
    final conflicts = outbox.where((entry) => entry.conflicted).toList();
    if (conflicts.isEmpty) {
      await ref.read(mkIdeasProvider.notifier).retryOutbox();
      ref.invalidate(mkIdeasProvider);
      return;
    }
    await showModalBottomSheet<void>(
      context: context,
      showDragHandle: true,
      builder: (sheetContext) => SafeArea(
        child: Padding(
          padding: const EdgeInsets.fromLTRB(
            Grid.gutter,
            0,
            Grid.gutter,
            Grid.gutter,
          ),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Text(
                'Review local changes',
                style: context.textTheme.titleMedium,
              ),
              const SizedBox(height: Grid.xxs),
              Text(
                'These changes conflict with newer shared data. Reapply keeps your change on top; discard removes the local draft.',
                style: context.textTheme.bodySmall,
              ),
              const SizedBox(height: Grid.sm),
              for (final entry in conflicts)
                _ConflictRow(
                  entry: entry,
                  onReapply: () async {
                    await ref
                        .read(mkIdeasProvider.notifier)
                        .reapplyOutboxEntry(entry);
                    if (sheetContext.mounted) Navigator.pop(sheetContext);
                  },
                  onDiscard: () async {
                    final confirmed = await showDialog<bool>(
                      context: sheetContext,
                      builder: (dialogContext) => AlertDialog(
                        title: const Text('Discard local change?'),
                        content: const Text(
                          'This removes the saved local draft and cannot be undone.',
                        ),
                        actions: [
                          TextButton(
                            onPressed: () =>
                                Navigator.pop(dialogContext, false),
                            child: const Text('Keep'),
                          ),
                          FilledButton(
                            onPressed: () => Navigator.pop(dialogContext, true),
                            child: const Text('Discard'),
                          ),
                        ],
                      ),
                    );
                    if (confirmed != true) return;
                    await ref
                        .read(mkIdeasProvider.notifier)
                        .discardOutboxEntry(entry.id);
                    if (sheetContext.mounted) Navigator.pop(sheetContext);
                  },
                ),
            ],
          ),
        ),
      ),
    );
  }
}

class _ConflictRow extends StatelessWidget {
  const _ConflictRow({
    required this.entry,
    required this.onReapply,
    required this.onDiscard,
  });

  final MkOutboxEntry entry;
  final Future<void> Function() onReapply;
  final Future<void> Function() onDiscard;

  @override
  Widget build(BuildContext context) {
    final label =
        entry.fields['title'] ?? entry.fields['name'] ?? entry.type.label;
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(Grid.xs),
        child: Row(
          children: [
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text('$label', style: context.textTheme.labelLarge),
                  Text(
                    entry.error ?? 'A newer shared version exists.',
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                    style: context.textTheme.bodySmall,
                  ),
                ],
              ),
            ),
            TextButton(onPressed: onDiscard, child: const Text('Discard')),
            FilledButton(onPressed: onReapply, child: const Text('Reapply')),
          ],
        ),
      ),
    );
  }
}
