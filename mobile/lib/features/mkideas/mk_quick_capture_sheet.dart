import 'package:flutter/material.dart';
import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../../shared/mkideas/mkideas_provider.dart';
import '../../shared/theme/theme.dart';

Future<bool?> showMkQuickCaptureSheet(
  BuildContext context, {
  MkCaptureType initialType = MkCaptureType.task,
  Map<String, dynamic> contextFields = const {},
}) => showModalBottomSheet<bool>(
  context: context,
  isScrollControlled: true,
  useSafeArea: true,
  builder: (_) => MkQuickCaptureSheet(
    initialType: initialType,
    contextFields: contextFields,
  ),
);

class MkQuickCaptureSheet extends HookConsumerWidget {
  const MkQuickCaptureSheet({
    this.initialType = MkCaptureType.task,
    this.contextFields = const {},
    super.key,
  });

  final MkCaptureType initialType;
  final Map<String, dynamic> contextFields;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final type = useState(initialType);
    final title = useTextEditingController();
    final details = useTextEditingController();
    final organization = useTextEditingController();
    final whyNow = useTextEditingController();
    final isSaving = useState(false);
    final error = useState<String?>(null);

    Future<void> save() async {
      if (title.text.trim().isEmpty) {
        error.value = 'Add a title before saving.';
        return;
      }
      isSaving.value = true;
      error.value = null;
      try {
        await ref
            .read(mkIdeasProvider.notifier)
            .capture(
              MkCaptureDraft(
                type: type.value,
                title: title.text,
                details: details.text,
                organization: organization.text,
                whyNow: whyNow.text,
                contextFields: contextFields,
              ),
            );
        if (context.mounted) Navigator.of(context).pop(true);
      } catch (exception) {
        error.value = 'Could not save this capture. $exception';
      } finally {
        if (context.mounted) isSaving.value = false;
      }
    }

    return Padding(
      padding: EdgeInsets.fromLTRB(
        Grid.gutter,
        Grid.lg,
        Grid.gutter,
        MediaQuery.viewInsetsOf(context).bottom + Grid.lg,
      ),
      child: SingleChildScrollView(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Row(
              children: [
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        'QUICK CAPTURE',
                        style: context.textTheme.labelSmall?.copyWith(
                          color: context.colors.primary,
                          fontWeight: FontWeight.w700,
                          letterSpacing: 1.4,
                        ),
                      ),
                      Text(
                        'Catch it while it is fresh',
                        style: context.textTheme.headlineSmall?.copyWith(
                          fontFamily: 'Georgia',
                          fontWeight: FontWeight.w700,
                        ),
                      ),
                    ],
                  ),
                ),
                IconButton(
                  onPressed: isSaving.value
                      ? null
                      : () => Navigator.of(context).pop(false),
                  icon: const Icon(LucideIcons.x),
                  tooltip: 'Close',
                ),
              ],
            ),
            const SizedBox(height: Grid.md),
            DropdownButtonFormField<MkCaptureType>(
              key: const Key('mk-quick-capture-type'),
              initialValue: type.value,
              decoration: const InputDecoration(labelText: 'Capture type'),
              items: [
                for (final captureType in MkCaptureType.values)
                  DropdownMenuItem(
                    value: captureType,
                    child: Text(captureType.label),
                  ),
              ],
              onChanged: isSaving.value
                  ? null
                  : (value) {
                      if (value != null) type.value = value;
                    },
            ),
            const SizedBox(height: Grid.xs),
            TextField(
              key: const Key('mk-quick-capture-title'),
              controller: title,
              autofocus: true,
              textCapitalization: TextCapitalization.sentences,
              decoration: InputDecoration(labelText: _titleLabel(type.value)),
              onSubmitted: (_) {
                if (!isSaving.value) save();
              },
            ),
            if (type.value == MkCaptureType.guest) ...[
              const SizedBox(height: Grid.xs),
              TextField(
                key: const Key('mk-quick-capture-organization'),
                controller: organization,
                textCapitalization: TextCapitalization.words,
                decoration: const InputDecoration(labelText: 'Organization'),
              ),
              const SizedBox(height: Grid.xs),
              TextField(
                key: const Key('mk-quick-capture-why-now'),
                controller: whyNow,
                textCapitalization: TextCapitalization.sentences,
                decoration: const InputDecoration(labelText: 'Why now?'),
              ),
            ] else ...[
              const SizedBox(height: Grid.xs),
              TextField(
                key: const Key('mk-quick-capture-details'),
                controller: details,
                minLines: 2,
                maxLines: 5,
                textCapitalization: TextCapitalization.sentences,
                decoration: InputDecoration(
                  labelText: type.value == MkCaptureType.knowledgeNote
                      ? 'Note'
                      : 'Details',
                ),
              ),
            ],
            if (error.value case final message?) ...[
              const SizedBox(height: Grid.xs),
              Container(
                padding: const EdgeInsets.all(Grid.xs),
                decoration: BoxDecoration(
                  color: context.colors.errorContainer,
                  borderRadius: BorderRadius.circular(Radii.md),
                ),
                child: Text(
                  message,
                  key: const Key('mk-quick-capture-error'),
                  style: context.textTheme.bodySmall?.copyWith(
                    color: context.colors.onErrorContainer,
                  ),
                ),
              ),
            ],
            const SizedBox(height: Grid.md),
            FilledButton.icon(
              key: const Key('mk-quick-capture-save'),
              onPressed: isSaving.value ? null : save,
              icon: isSaving.value
                  ? const SizedBox.square(
                      dimension: 16,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Icon(LucideIcons.plus, size: 18),
              label: Text(isSaving.value ? 'Saving…' : 'Save to MK Ideas'),
            ),
          ],
        ),
      ),
    );
  }
}

String _titleLabel(MkCaptureType type) => switch (type) {
  MkCaptureType.guest => 'Guest name',
  MkCaptureType.task => 'Task',
  MkCaptureType.meeting => 'Meeting',
  MkCaptureType.contentIdea => 'Content idea',
  MkCaptureType.knowledgeNote => 'Note title',
};
