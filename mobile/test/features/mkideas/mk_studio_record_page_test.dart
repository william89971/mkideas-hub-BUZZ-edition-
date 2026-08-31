import 'package:buzz/features/mkideas/mk_studio_record_page.dart';
import 'package:buzz/shared/mkideas/mkideas_provider.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import '../../helpers/widget_helpers.dart';

void main() {
  testWidgets('shows immutable transcript metadata and human-gated clips', (
    tester,
  ) async {
    final interview = _interview();
    final proposal = MkProposal(
      eventId: 'proposal-event',
      proposalId: 'proposal-id',
      targetId: interview.entityId,
      targetKind: interview.kind,
      agent: 'Content / Clip Copilot',
      summary: 'One timestamped clip is ready.',
      provenance: const ['Private transcript'],
      clips: const [
        {
          'start_ms': 4000,
          'end_ms': 18000,
          'title': 'Human judgment',
          'caption': 'A draft caption.',
        },
      ],
      createdAt: 2,
      schemaVersion: 2,
      personaId: MkAgentPersona.contentClipCopilot.id,
      targetEventId: interview.eventId,
      targetVersion: interview.version,
    );
    final notifier = _FakeMkIdeasNotifier(
      MkIdeasSnapshot.fromRecords(records: [interview], proposals: [proposal]),
    );

    await tester.pumpWidget(
      WidgetHelpers.testable(
        overrides: [mkIdeasProvider.overrideWith(() => notifier)],
        child: MkStudioRecordPage(record: interview),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.byKey(const Key('mk-transcript-descriptor')), findsOneWidget);
    expect(find.text('interview.vtt'), findsOneWidget);
    expect(find.textContaining('private media'), findsOneWidget);
    await tester.scrollUntilVisible(
      find.text('DRAFT · HUMAN DECISION REQUIRED'),
      300,
    );
    expect(find.textContaining('0:04–0:18'), findsOneWidget);
    expect(find.text('DRAFT · HUMAN DECISION REQUIRED'), findsOneWidget);
    expect(find.text('Approve'), findsOneWidget);
  });
}

MkRecord _interview() => MkRecord(
  eventId: 'interview-event',
  author: 'human',
  kind: MkEntityType.interview.kind,
  createdAt: 1,
  entityId: 'interview-id',
  version: 3,
  schemaVersion: 2,
  type: MkEntityType.interview,
  payload: const MkInterviewData({
    'title': 'Interview with Avery',
    'transcript_media': {
      'media_id': 'private-media-id',
      'sha256':
          'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
      'mime_type': 'text/vtt',
      'size_bytes': 2048,
      'original_filename': 'interview.vtt',
      'version': 2,
    },
  }, MkInterviewStatus.reviewing),
);

class _FakeMkIdeasNotifier extends MkIdeasNotifier {
  _FakeMkIdeasNotifier(this.snapshot);
  final MkIdeasSnapshot snapshot;

  @override
  Future<MkIdeasSnapshot> build() async => snapshot;

  @override
  Future<MkEntityHistoryPage> loadHistoryPage(
    MkEntityCoordinate coordinate, {
    String? cursor,
  }) async => MkEntityHistoryPage(
    records: snapshot.revisionsFor(coordinate.type, coordinate.entityId),
  );
}
