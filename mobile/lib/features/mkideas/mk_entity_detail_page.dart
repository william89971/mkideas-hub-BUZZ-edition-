import 'package:flutter/material.dart';
import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:flutter/services.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../../shared/deeplink/deep_link.dart';
import '../../shared/mkideas/mkideas_provider.dart';
import '../../shared/relay/relay_provider.dart';
import '../../shared/theme/theme.dart';

/// A cross-area record view used by search and canonical MK Ideas links.
class MkEntityDetailPage extends HookConsumerWidget {
  const MkEntityDetailPage({
    required this.initialRecord,
    this.community,
    this.revisionEventId,
    super.key,
  });

  final MkRecord initialRecord;
  final String? community;
  final String? revisionEventId;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final snapshot = ref.watch(mkIdeasProvider);
    final data = snapshot.asData?.value;
    final revisions =
        data?.revisionsFor(initialRecord.type, initialRecord.entityId) ??
        const <MkRecord>[];
    final record = _resolveRecord(initialRecord, revisions, revisionEventId);
    final isHistorical =
        revisionEventId != null &&
        record.eventId == revisionEventId &&
        revisions.isNotEmpty &&
        revisions.last.eventId != record.eventId;
    final relayHost =
        community ?? mkCommunityHost(ref.watch(relayConfigProvider).wsUrl);
    final canonicalLink = buildMkIdeasEntityLink(
      community: relayHost,
      kind: record.kind,
      entityId: record.entityId,
      eventId: isHistorical ? record.eventId : null,
    );
    final historyCursor = useState<String?>(null);
    final historyLoading = useState(false);
    final historyError = useState<String?>(null);

    Future<void> loadHistory({String? cursor}) async {
      if (historyLoading.value) return;
      historyLoading.value = true;
      historyError.value = null;
      try {
        final page = await ref
            .read(mkIdeasProvider.notifier)
            .loadHistoryPage(initialRecord.coordinate, cursor: cursor);
        if (!context.mounted) return;
        historyCursor.value = page.nextCursor;
      } catch (error) {
        if (!context.mounted) return;
        historyError.value = '$error';
      } finally {
        if (context.mounted) historyLoading.value = false;
      }
    }

    useEffect(() {
      Future<void>(() => loadHistory());
      return null;
    }, [initialRecord.coordinate]);

    return Scaffold(
      backgroundColor: context.colors.surface,
      appBar: AppBar(
        title: Text(record.type.label),
        actions: [
          IconButton(
            key: const Key('mk-entity-copy-link'),
            onPressed: () async {
              await Clipboard.setData(ClipboardData(text: canonicalLink));
              if (!context.mounted) return;
              ScaffoldMessenger.of(context).showSnackBar(
                const SnackBar(content: Text('MK Ideas link copied')),
              );
            },
            icon: const Icon(LucideIcons.link),
            tooltip: 'Copy stable link',
          ),
        ],
      ),
      body: SafeArea(
        top: false,
        child: ListView(
          padding: const EdgeInsets.fromLTRB(
            Grid.gutter,
            Grid.md,
            Grid.gutter,
            Grid.xl,
          ),
          children: [
            Text(
              record.type.productArea.name.toUpperCase(),
              style: context.textTheme.labelSmall?.copyWith(
                color: context.colors.primary,
                fontWeight: FontWeight.w700,
                letterSpacing: 1.4,
              ),
            ),
            const SizedBox(height: Grid.half),
            Text(
              record.title,
              key: const Key('mk-entity-title'),
              style: context.textTheme.headlineMedium?.copyWith(
                fontFamily: 'Georgia',
                fontWeight: FontWeight.w700,
              ),
            ),
            const SizedBox(height: Grid.xs),
            Wrap(
              spacing: Grid.xxs,
              runSpacing: Grid.xxs,
              children: [
                _MetadataPill(label: record.status.replaceAll('-', ' ')),
                _MetadataPill(label: 'Version ${record.version}'),
                if (isHistorical)
                  const _MetadataPill(label: 'Historical revision'),
              ],
            ),
            if (data?.syncIssue case final issue?) ...[
              const SizedBox(height: Grid.xs),
              _Notice(message: issue),
            ],
            const SizedBox(height: Grid.lg),
            Text(
              'DETAILS',
              style: context.textTheme.labelSmall?.copyWith(
                color: context.colors.primary,
                fontWeight: FontWeight.w700,
                letterSpacing: 1.4,
              ),
            ),
            const SizedBox(height: Grid.xxs),
            ..._visibleDetails(record).map(
              (detail) => _DetailRow(label: detail.label, value: detail.value),
            ),
            const SizedBox(height: Grid.lg),
            Text(
              'HISTORY',
              style: context.textTheme.labelSmall?.copyWith(
                color: context.colors.primary,
                fontWeight: FontWeight.w700,
                letterSpacing: 1.4,
              ),
            ),
            const SizedBox(height: Grid.xxs),
            if (revisions.isEmpty)
              _Notice(
                message:
                    historyError.value ??
                    (historyLoading.value
                        ? 'Revision history is loading.'
                        : 'No signed history is available.'),
              )
            else
              for (final revision in revisions.reversed)
                ListTile(
                  key: ValueKey('mk-entity-revision-${revision.eventId}'),
                  contentPadding: EdgeInsets.zero,
                  leading: Icon(
                    revision.eventId == record.eventId
                        ? LucideIcons.circleCheckBig
                        : LucideIcons.history,
                    size: 18,
                    color: context.colors.primary,
                  ),
                  title: Text(
                    'Version ${revision.version} · '
                    '${revision.status.replaceAll('-', ' ')}',
                  ),
                  subtitle: Text(
                    'Signed by ${_shortAuthor(revision.author)}',
                    style: context.textTheme.bodySmall?.copyWith(
                      color: context.colors.onSurfaceVariant,
                    ),
                  ),
                ),
            if (historyCursor.value != null) ...[
              const SizedBox(height: Grid.sm),
              OutlinedButton.icon(
                key: const Key('mk-entity-load-older-history'),
                onPressed: historyLoading.value
                    ? null
                    : () => loadHistory(cursor: historyCursor.value),
                icon: const Icon(LucideIcons.history, size: 16),
                label: Text(
                  historyLoading.value ? 'Loading…' : 'Load older history',
                ),
              ),
            ],
          ],
        ),
      ),
    );
  }
}

MkRecord _resolveRecord(
  MkRecord fallback,
  List<MkRecord> revisions,
  String? revisionEventId,
) {
  if (revisionEventId != null) {
    for (final record in revisions) {
      if (record.eventId == revisionEventId) return record;
    }
    if (fallback.eventId == revisionEventId) return fallback;
  }
  return revisions.isEmpty ? fallback : revisions.last;
}

List<({String label, String value})> _visibleDetails(MkRecord record) {
  const labels = {
    'description': 'Description',
    'organization': 'Organization',
    'title': 'Title',
    'name': 'Name',
    'why_now': 'Why now',
    'body': 'Note',
    'deadline': 'Deadline',
    'due_at': 'Due',
    'scheduled_at': 'Scheduled',
    'do_not_contact': 'Do not contact',
  };
  final details = <({String label, String value})>[];
  for (final entry in labels.entries) {
    final value = record.data[entry.key];
    if (value == null || value is String && value.trim().isEmpty) continue;
    final rendered = value is bool ? (value ? 'Yes' : 'No') : '$value';
    details.add((label: entry.value, value: rendered));
  }
  if (details.isEmpty) {
    details.add((label: 'Status', value: record.status.replaceAll('-', ' ')));
  }
  return details;
}

String _shortAuthor(String author) => author.length <= 12
    ? author
    : '${author.substring(0, 6)}…${author.substring(author.length - 4)}';

class _MetadataPill extends StatelessWidget {
  const _MetadataPill({required this.label});

  final String label;

  @override
  Widget build(BuildContext context) => Container(
    padding: const EdgeInsets.symmetric(
      horizontal: Grid.xxs,
      vertical: Grid.half,
    ),
    decoration: BoxDecoration(
      color: context.colors.surfaceContainerLow,
      border: Border.all(color: context.colors.outlineVariant),
      borderRadius: BorderRadius.circular(Radii.full),
    ),
    child: Text(label, style: context.textTheme.labelSmall),
  );
}

class _DetailRow extends StatelessWidget {
  const _DetailRow({required this.label, required this.value});

  final String label;
  final String value;

  @override
  Widget build(BuildContext context) => Container(
    padding: const EdgeInsets.symmetric(vertical: Grid.xs),
    decoration: BoxDecoration(
      border: Border(bottom: BorderSide(color: context.colors.outlineVariant)),
    ),
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          label,
          style: context.textTheme.labelSmall?.copyWith(
            color: context.colors.onSurfaceVariant,
          ),
        ),
        const SizedBox(height: Grid.half),
        SelectableText(value, style: context.textTheme.bodyMedium),
      ],
    ),
  );
}

class _Notice extends StatelessWidget {
  const _Notice({required this.message});

  final String message;

  @override
  Widget build(BuildContext context) => Container(
    padding: const EdgeInsets.all(Grid.xs),
    decoration: BoxDecoration(
      color: context.colors.surfaceContainerLow,
      borderRadius: BorderRadius.circular(Radii.md),
    ),
    child: Text(
      message,
      style: context.textTheme.bodySmall?.copyWith(
        color: context.colors.onSurfaceVariant,
      ),
    ),
  );
}
