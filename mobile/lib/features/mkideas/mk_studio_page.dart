import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../../shared/deeplink/deep_link.dart';
import '../../shared/mkideas/mkideas_provider.dart';
import '../../shared/relay/media_upload.dart';
import '../../shared/relay/relay_provider.dart';
import '../../shared/theme/theme.dart';
import 'mk_studio_record_page.dart';

class MkStudioPage extends ConsumerWidget {
  const MkStudioPage({required this.onSearch, super.key});

  final VoidCallback onSearch;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final snapshot = ref.watch(mkIdeasProvider);
    final community = mkCommunityHost(ref.watch(relayConfigProvider).wsUrl);
    return Scaffold(
      backgroundColor: context.colors.surface,
      body: SafeArea(
        child: snapshot.when(
          loading: () => const Center(child: CircularProgressIndicator()),
          error: (error, _) => Center(child: Text('$error')),
          data: (data) {
            final interviews = data.recordsOfType(MkEntityType.interview);
            final content = data.recordsOfType(MkEntityType.content);
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
                  _Header(onSearch: onSearch),
                  const SizedBox(height: Grid.lg),
                  const _SectionLabel('Interviews'),
                  const SizedBox(height: Grid.sm),
                  if (interviews.isEmpty)
                    const _Panel(
                      text: 'Create an interview from a guest in People.',
                    )
                  else
                    for (final interview in interviews)
                      _StudioRecordCard(
                        record: interview,
                        descriptor:
                            (interview.payload as MkInterviewData).transcript,
                        onOpen: () => _open(context, interview),
                        onUpload: () =>
                            _attachTranscript(context, ref, interview),
                        onCreateContent: () => ref
                            .read(mkIdeasProvider.notifier)
                            .createContent(interview),
                        onCopy: () => _copyReference(community, interview),
                      ),
                  const SizedBox(height: Grid.xl),
                  const _SectionLabel('Content desk'),
                  const SizedBox(height: Grid.sm),
                  if (content.isEmpty)
                    const _Panel(
                      text:
                          'Approved ideas and publication tracking appear here.',
                    )
                  else
                    for (final item in content)
                      _StudioRecordCard(
                        record: item,
                        onOpen: () => _open(context, item),
                        onCopy: () => _copyReference(community, item),
                      ),
                  const SizedBox(height: Grid.xl),
                  const _Panel(
                    text:
                        'AI proposes clips, captions, and preparation. '
                        'A partner must review every protected change. '
                        'Buzz stores transcript descriptors, not oversized transcript events.',
                  ),
                ],
              ),
            );
          },
        ),
      ),
    );
  }
}

void _open(BuildContext context, MkRecord record) {
  Navigator.of(context).push(
    MaterialPageRoute<void>(builder: (_) => MkStudioRecordPage(record: record)),
  );
}

Future<void> _attachTranscript(
  BuildContext context,
  WidgetRef ref,
  MkRecord interview,
) async {
  try {
    final service = ref.read(mediaUploadServiceProvider);
    final file = await service.pickAttachmentFile();
    if (file == null) return;
    final extension = file.name.split('.').last.toLowerCase();
    if (!const {'txt', 'vtt', 'srt'}.contains(extension)) {
      throw const FormatException('Choose a TXT, VTT, or SRT transcript.');
    }
    final uploaded = await service.uploadFile(file);
    final currentDescriptor = (interview.payload as MkInterviewData).transcript;
    final descriptor = MkMediaDescriptor(
      mediaId: uploaded.sha256,
      sha256: uploaded.sha256,
      mimeType: switch (extension) {
        'vtt' => 'text/vtt',
        'srt' => 'application/x-subrip',
        _ => 'text/plain',
      },
      sizeBytes: uploaded.size,
      filename: uploaded.filename ?? file.name,
      version: (currentDescriptor?.version ?? 0) + 1,
      uploadedAt: DateTime.now().toUtc().toIso8601String(),
      source: 'buzz-mobile-private-media',
      format: extension,
    );
    await ref
        .read(mkIdeasProvider.notifier)
        .attachTranscriptDescriptor(interview, descriptor);
    if (!context.mounted) return;
    ScaffoldMessenger.of(context).showSnackBar(
      const SnackBar(content: Text('Private transcript attached')),
    );
  } catch (error) {
    if (!context.mounted) return;
    ScaffoldMessenger.of(
      context,
    ).showSnackBar(SnackBar(content: Text('$error')));
  }
}

Future<void> _copyReference(
  String community,
  MkRecord record,
) => Clipboard.setData(
  ClipboardData(
    text:
        '${record.title} — ${buildMkIdeasEntityLink(community: community, kind: record.kind, entityId: record.entityId)}',
  ),
);

class _StudioRecordCard extends StatelessWidget {
  const _StudioRecordCard({
    required this.record,
    required this.onOpen,
    required this.onCopy,
    this.descriptor,
    this.onUpload,
    this.onCreateContent,
  });

  final MkRecord record;
  final MkMediaDescriptor? descriptor;
  final VoidCallback onOpen;
  final VoidCallback onCopy;
  final VoidCallback? onUpload;
  final VoidCallback? onCreateContent;

  @override
  Widget build(BuildContext context) => Card(
    margin: const EdgeInsets.only(bottom: Grid.sm),
    child: InkWell(
      onTap: onOpen,
      borderRadius: BorderRadius.circular(Radii.md),
      child: Padding(
        padding: const EdgeInsets.all(Grid.md),
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
            Text(
              '${record.status.replaceAll('-', ' ')} · version ${record.version}',
              style: context.textTheme.bodySmall?.copyWith(
                color: context.colors.onSurfaceVariant,
              ),
            ),
            if (descriptor != null) ...[
              const SizedBox(height: Grid.xs),
              Text(
                '${descriptor!.filename} · media v${descriptor!.version}',
                key: const Key('mk-studio-transcript-summary'),
                style: context.textTheme.labelSmall?.copyWith(
                  color: context.colors.primary,
                ),
              ),
            ],
            const SizedBox(height: Grid.sm),
            Wrap(
              spacing: Grid.sm,
              runSpacing: Grid.sm,
              children: [
                if (onUpload != null)
                  OutlinedButton.icon(
                    onPressed: onUpload,
                    icon: const Icon(LucideIcons.upload, size: 16),
                    label: Text(
                      descriptor == null
                          ? 'Upload transcript'
                          : 'Replace transcript',
                    ),
                  ),
                if (onCreateContent != null)
                  OutlinedButton.icon(
                    onPressed: onCreateContent,
                    icon: const Icon(LucideIcons.fileText, size: 16),
                    label: const Text('Create content'),
                  ),
                IconButton(
                  onPressed: onCopy,
                  icon: const Icon(LucideIcons.copy),
                  tooltip: 'Copy Team reference',
                ),
              ],
            ),
          ],
        ),
      ),
    ),
  );
}

class _Header extends StatelessWidget {
  const _Header({required this.onSearch});
  final VoidCallback onSearch;

  @override
  Widget build(BuildContext context) => Row(
    children: [
      Expanded(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const _SectionLabel('Interview to idea'),
            Text(
              'Studio',
              style: context.textTheme.headlineLarge?.copyWith(
                fontFamily: 'Georgia',
                fontWeight: FontWeight.w700,
              ),
            ),
          ],
        ),
      ),
      IconButton(onPressed: onSearch, icon: const Icon(LucideIcons.search)),
    ],
  );
}

class _SectionLabel extends StatelessWidget {
  const _SectionLabel(this.text);
  final String text;

  @override
  Widget build(BuildContext context) => Text(
    text.toUpperCase(),
    style: context.textTheme.labelSmall?.copyWith(
      color: context.colors.primary,
      letterSpacing: 1.4,
      fontWeight: FontWeight.w700,
    ),
  );
}

class _Panel extends StatelessWidget {
  const _Panel({required this.text});
  final String text;

  @override
  Widget build(BuildContext context) => Container(
    padding: const EdgeInsets.all(Grid.lg),
    decoration: BoxDecoration(
      border: Border.all(color: context.colors.outlineVariant),
      borderRadius: BorderRadius.circular(Radii.md),
    ),
    child: Text(
      text,
      textAlign: TextAlign.center,
      style: context.textTheme.bodyMedium?.copyWith(
        color: context.colors.onSurfaceVariant,
      ),
    ),
  );
}
