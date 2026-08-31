import 'package:flutter/material.dart';
import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../../shared/mkideas/mkideas_provider.dart';
import '../../shared/theme/theme.dart';
import 'mk_quick_capture_sheet.dart';

class MkWorkPage extends HookConsumerWidget {
  const MkWorkPage({required this.onSearch, super.key});

  final VoidCallback onSearch;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final snapshot = ref.watch(mkIdeasProvider);
    final selectedType = useState<MkEntityType?>(null);
    final query = useState('');

    return Scaffold(
      backgroundColor: context.colors.surface,
      body: SafeArea(
        child: snapshot.when(
          loading: () => const Center(child: CircularProgressIndicator()),
          error: (error, _) => _WorkMessage(
            icon: LucideIcons.triangleAlert,
            title: 'Work could not load',
            body: '$error',
          ),
          data: (data) {
            final allWork = data.records
                .where((record) => _workTypes.contains(record.type))
                .toList(growable: false);
            final work = allWork
                .where(
                  (record) =>
                      selectedType.value == null ||
                      record.type == selectedType.value,
                )
                .where((record) {
                  final needle = query.value.trim().toLowerCase();
                  if (needle.isEmpty) return true;
                  return record.title.toLowerCase().contains(needle) ||
                      record.status.toLowerCase().contains(needle);
                })
                .toList(growable: false);
            final activeTasks = allWork.where(
              (record) =>
                  record.type == MkEntityType.task &&
                  record.status != MkTaskStatus.done.wireName &&
                  record.status != MkTaskStatus.cancelled.wireName,
            );
            final activeProjects = allWork.where(
              (record) =>
                  record.type == MkEntityType.operationalProject &&
                  record.status == MkProjectStatus.active.wireName,
            );
            final plannedMeetings = allWork.where(
              (record) =>
                  record.type == MkEntityType.meeting &&
                  record.status == MkMeetingStatus.planned.wireName,
            );

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
                  _WorkHeader(
                    onSearch: onSearch,
                    onCreate: (action) => _handleCreateAction(context, action),
                  ),
                  if (data.syncIssue case final issue?) ...[
                    const SizedBox(height: Grid.xs),
                    _SyncNotice(message: issue),
                  ],
                  const SizedBox(height: Grid.md),
                  Row(
                    children: [
                      Expanded(
                        child: _WorkMetric(
                          value: '${activeTasks.length}',
                          label: 'Open tasks',
                        ),
                      ),
                      const SizedBox(width: Grid.xxs),
                      Expanded(
                        child: _WorkMetric(
                          value: '${activeProjects.length}',
                          label: 'Active projects',
                        ),
                      ),
                      const SizedBox(width: Grid.xxs),
                      Expanded(
                        child: _WorkMetric(
                          value: '${plannedMeetings.length}',
                          label: 'Meetings',
                        ),
                      ),
                    ],
                  ),
                  const SizedBox(height: Grid.md),
                  TextField(
                    key: const Key('mk-work-filter'),
                    onChanged: (value) => query.value = value,
                    decoration: const InputDecoration(
                      prefixIcon: Icon(LucideIcons.search),
                      hintText: 'Filter work',
                    ),
                  ),
                  const SizedBox(height: Grid.xs),
                  SingleChildScrollView(
                    scrollDirection: Axis.horizontal,
                    child: Row(
                      children: [
                        ChoiceChip(
                          label: const Text('All'),
                          selected: selectedType.value == null,
                          onSelected: (_) => selectedType.value = null,
                        ),
                        for (final type in _workTypes) ...[
                          const SizedBox(width: Grid.xxs),
                          ChoiceChip(
                            label: Text(type.label),
                            selected: selectedType.value == type,
                            onSelected: (_) => selectedType.value = type,
                          ),
                        ],
                      ],
                    ),
                  ),
                  const SizedBox(height: Grid.md),
                  Row(
                    children: [
                      Expanded(
                        child: Text(
                          'OPERATING BOARD',
                          style: context.textTheme.labelSmall?.copyWith(
                            color: context.colors.primary,
                            fontWeight: FontWeight.w700,
                            letterSpacing: 1.4,
                          ),
                        ),
                      ),
                      Text(
                        '${work.length} current',
                        style: context.textTheme.bodySmall?.copyWith(
                          color: context.colors.onSurfaceVariant,
                        ),
                      ),
                    ],
                  ),
                  const SizedBox(height: Grid.xxs),
                  if (work.isEmpty)
                    _WorkMessage(
                      icon: LucideIcons.listChecks,
                      title: allWork.isEmpty
                          ? 'Build the first operating plan'
                          : 'No work matches this view',
                      body: allWork.isEmpty
                          ? 'Capture a task or meeting, or use Create for a '
                                'goal, project, or decision.'
                          : 'Change the type or search filter to see more work.',
                    )
                  else
                    for (final record in work) _WorkRecordCard(record: record),
                ],
              ),
            );
          },
        ),
      ),
    );
  }
}

const _workTypes = [
  MkEntityType.goal,
  MkEntityType.operationalProject,
  MkEntityType.task,
  MkEntityType.meeting,
  MkEntityType.decision,
];

enum _WorkCreateAction { quickTask, quickMeeting, goal, project, decision }

Future<void> _handleCreateAction(
  BuildContext context,
  _WorkCreateAction action,
) async {
  switch (action) {
    case _WorkCreateAction.quickTask:
      await showMkQuickCaptureSheet(context);
    case _WorkCreateAction.quickMeeting:
      await showMkQuickCaptureSheet(
        context,
        initialType: MkCaptureType.meeting,
      );
    case _WorkCreateAction.goal:
      await _showTypedWorkSheet(
        context,
        type: MkEntityType.goal,
        status: MkGoalStatus.draft,
      );
    case _WorkCreateAction.project:
      await _showTypedWorkSheet(
        context,
        type: MkEntityType.operationalProject,
        status: MkProjectStatus.planned,
      );
    case _WorkCreateAction.decision:
      await _showTypedWorkSheet(
        context,
        type: MkEntityType.decision,
        status: MkDecisionStatus.proposed,
      );
  }
}

Future<void> _showTypedWorkSheet(
  BuildContext context, {
  required MkEntityType type,
  required MkStatusValue status,
}) => showModalBottomSheet<void>(
  context: context,
  isScrollControlled: true,
  useSafeArea: true,
  builder: (_) => _TypedWorkSheet(type: type, status: status),
);

class _TypedWorkSheet extends HookConsumerWidget {
  const _TypedWorkSheet({required this.type, required this.status});

  final MkEntityType type;
  final MkStatusValue status;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final title = useTextEditingController();
    final description = useTextEditingController();
    final saving = useState(false);
    final error = useState<String?>(null);

    Future<void> save() async {
      if (title.text.trim().isEmpty) return;
      saving.value = true;
      error.value = null;
      try {
        await ref
            .read(mkIdeasProvider.notifier)
            .createEntity(
              type: type,
              status: status,
              title: title.text,
              description: description.text,
            );
        if (context.mounted) Navigator.of(context).pop();
      } catch (exception) {
        error.value =
            'Could not create this ${type.label.toLowerCase()}. $exception';
      } finally {
        if (context.mounted) saving.value = false;
      }
    }

    return Padding(
      padding: EdgeInsets.fromLTRB(
        Grid.gutter,
        Grid.lg,
        Grid.gutter,
        MediaQuery.viewInsetsOf(context).bottom + Grid.lg,
      ),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Text(
            'New ${type.label.toLowerCase()}',
            style: context.textTheme.headlineSmall?.copyWith(
              fontFamily: 'Georgia',
              fontWeight: FontWeight.w700,
            ),
          ),
          const SizedBox(height: Grid.md),
          TextField(
            controller: title,
            autofocus: true,
            decoration: InputDecoration(labelText: '${type.label} title'),
          ),
          const SizedBox(height: Grid.xs),
          TextField(
            controller: description,
            minLines: 2,
            maxLines: 4,
            decoration: const InputDecoration(labelText: 'Details'),
          ),
          if (error.value case final message?) ...[
            const SizedBox(height: Grid.xs),
            Text(
              message,
              style: context.textTheme.bodySmall?.copyWith(
                color: context.colors.error,
              ),
            ),
          ],
          const SizedBox(height: Grid.md),
          FilledButton(
            onPressed: saving.value ? null : save,
            child: Text(saving.value ? 'Saving…' : 'Create ${type.label}'),
          ),
        ],
      ),
    );
  }
}

class _WorkHeader extends StatelessWidget {
  const _WorkHeader({required this.onSearch, required this.onCreate});

  final VoidCallback onSearch;
  final ValueChanged<_WorkCreateAction> onCreate;

  @override
  Widget build(BuildContext context) => Row(
    crossAxisAlignment: CrossAxisAlignment.start,
    children: [
      Expanded(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              'OPERATING RHYTHM',
              style: context.textTheme.labelSmall?.copyWith(
                color: context.colors.primary,
                letterSpacing: 1.6,
                fontWeight: FontWeight.w700,
              ),
            ),
            Text(
              'Work',
              style: context.textTheme.headlineLarge?.copyWith(
                fontFamily: 'Georgia',
                fontWeight: FontWeight.w700,
              ),
            ),
            Text(
              'Goals become projects, tasks, meetings, and decisions.',
              style: context.textTheme.bodyMedium?.copyWith(
                color: context.colors.onSurfaceVariant,
              ),
            ),
          ],
        ),
      ),
      PopupMenuButton<_WorkCreateAction>(
        key: const Key('mk-work-create-menu'),
        tooltip: 'Create work',
        onSelected: onCreate,
        icon: const Icon(LucideIcons.circlePlus),
        itemBuilder: (_) => const [
          PopupMenuItem(
            value: _WorkCreateAction.quickTask,
            child: Text('Task'),
          ),
          PopupMenuItem(
            value: _WorkCreateAction.quickMeeting,
            child: Text('Meeting'),
          ),
          PopupMenuItem(value: _WorkCreateAction.goal, child: Text('Goal')),
          PopupMenuItem(
            value: _WorkCreateAction.project,
            child: Text('Project'),
          ),
          PopupMenuItem(
            value: _WorkCreateAction.decision,
            child: Text('Decision'),
          ),
        ],
      ),
      IconButton(onPressed: onSearch, icon: const Icon(LucideIcons.search)),
    ],
  );
}

class _WorkMetric extends StatelessWidget {
  const _WorkMetric({required this.value, required this.label});

  final String value;
  final String label;

  @override
  Widget build(BuildContext context) => Container(
    padding: const EdgeInsets.all(Grid.xs),
    decoration: BoxDecoration(
      border: Border.all(color: context.colors.outlineVariant),
      borderRadius: BorderRadius.circular(Radii.md),
    ),
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          value,
          style: context.textTheme.headlineSmall?.copyWith(
            fontFamily: 'Georgia',
            fontWeight: FontWeight.w700,
          ),
        ),
        Text(
          label,
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
          style: context.textTheme.labelSmall,
        ),
      ],
    ),
  );
}

class _WorkRecordCard extends ConsumerWidget {
  const _WorkRecordCard({required this.record});

  final MkRecord record;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final deadline = record.data['deadline'] ?? record.data['due_at'];
    final assignees = record.data['assignees'];
    return Card(
      key: ValueKey('mk-work-${record.entityId}'),
      margin: const EdgeInsets.only(bottom: Grid.xxs),
      child: Padding(
        padding: const EdgeInsets.all(Grid.xs),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Container(
              padding: const EdgeInsets.all(Grid.xxs),
              decoration: BoxDecoration(
                color: context.colors.primaryContainer,
                borderRadius: BorderRadius.circular(Radii.sm),
              ),
              child: Icon(
                _iconFor(record.type),
                size: 18,
                color: context.colors.onPrimaryContainer,
              ),
            ),
            const SizedBox(width: Grid.xs),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    record.title,
                    style: context.textTheme.titleMedium?.copyWith(
                      fontFamily: 'Georgia',
                      fontWeight: FontWeight.w700,
                    ),
                  ),
                  const SizedBox(height: Grid.half),
                  Wrap(
                    spacing: Grid.xxs,
                    runSpacing: Grid.half,
                    children: [
                      Text(
                        record.type.label.toUpperCase(),
                        style: context.textTheme.labelSmall?.copyWith(
                          color: context.colors.primary,
                          fontWeight: FontWeight.w700,
                        ),
                      ),
                      if (deadline != null)
                        Text(
                          'Due $deadline',
                          style: context.textTheme.bodySmall?.copyWith(
                            color: context.colors.onSurfaceVariant,
                          ),
                        ),
                      if (assignees is List && assignees.isNotEmpty)
                        Text(
                          '${assignees.length} assigned',
                          style: context.textTheme.bodySmall?.copyWith(
                            color: context.colors.onSurfaceVariant,
                          ),
                        ),
                    ],
                  ),
                ],
              ),
            ),
            PopupMenuButton<MkStatusValue>(
              key: ValueKey('mk-work-status-${record.entityId}'),
              tooltip: 'Change status',
              onSelected: (status) async {
                try {
                  await ref
                      .read(mkIdeasProvider.notifier)
                      .updateStatus(record, status);
                } catch (exception) {
                  if (!context.mounted) return;
                  ScaffoldMessenger.of(context).showSnackBar(
                    SnackBar(content: Text('Status change failed. $exception')),
                  );
                }
              },
              itemBuilder: (_) => [
                for (final status in record.type.statuses)
                  PopupMenuItem(
                    value: status,
                    child: Text(status.wireName.replaceAll('-', ' ')),
                  ),
              ],
              child: Container(
                padding: const EdgeInsets.symmetric(
                  horizontal: Grid.xxs,
                  vertical: Grid.half,
                ),
                decoration: BoxDecoration(
                  border: Border.all(color: context.colors.outlineVariant),
                  borderRadius: BorderRadius.circular(Radii.full),
                ),
                child: Text(
                  record.status.replaceAll('-', ' '),
                  style: context.textTheme.labelSmall,
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _WorkMessage extends StatelessWidget {
  const _WorkMessage({
    required this.icon,
    required this.title,
    required this.body,
  });

  final IconData icon;
  final String title;
  final String body;

  @override
  Widget build(BuildContext context) => Container(
    padding: const EdgeInsets.all(Grid.md),
    decoration: BoxDecoration(
      border: Border.all(color: context.colors.outlineVariant),
      borderRadius: BorderRadius.circular(Radii.md),
    ),
    child: Column(
      children: [
        Icon(icon, color: context.colors.primary),
        const SizedBox(height: Grid.xxs),
        Text(
          title,
          textAlign: TextAlign.center,
          style: context.textTheme.titleMedium?.copyWith(
            fontFamily: 'Georgia',
            fontWeight: FontWeight.w700,
          ),
        ),
        const SizedBox(height: Grid.half),
        Text(
          body,
          textAlign: TextAlign.center,
          style: context.textTheme.bodySmall?.copyWith(
            color: context.colors.onSurfaceVariant,
          ),
        ),
      ],
    ),
  );
}

class _SyncNotice extends StatelessWidget {
  const _SyncNotice({required this.message});

  final String message;

  @override
  Widget build(BuildContext context) => Container(
    padding: const EdgeInsets.all(Grid.xs),
    decoration: BoxDecoration(
      color: context.colors.errorContainer,
      borderRadius: BorderRadius.circular(Radii.md),
    ),
    child: Row(
      children: [
        Icon(
          LucideIcons.cloudOff,
          size: 18,
          color: context.colors.onErrorContainer,
        ),
        const SizedBox(width: Grid.xxs),
        Expanded(
          child: Text(
            message,
            style: context.textTheme.bodySmall?.copyWith(
              color: context.colors.onErrorContainer,
            ),
          ),
        ),
      ],
    ),
  );
}

IconData _iconFor(MkEntityType type) => switch (type) {
  MkEntityType.goal => LucideIcons.target,
  MkEntityType.operationalProject => LucideIcons.folderKanban,
  MkEntityType.task => LucideIcons.listChecks,
  MkEntityType.meeting => LucideIcons.calendarDays,
  MkEntityType.decision => LucideIcons.gitFork,
  _ => LucideIcons.circle,
};
