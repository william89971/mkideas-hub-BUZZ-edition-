import 'package:buzz/features/mkideas/mk_agent_panel.dart';
import 'package:buzz/shared/mkideas/mkideas_provider.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import '../../helpers/widget_helpers.dart';

void main() {
  testWidgets('shows every persona and the human-gate boundary', (
    tester,
  ) async {
    await tester.pumpWidget(
      WidgetHelpers.testable(
        child: SingleChildScrollView(
          child: Column(
            children: [
              for (final persona in MkAgentPersona.values)
                MkAgentPanel(
                  persona: persona,
                  snapshot: MkIdeasSnapshot.empty,
                  onLaunch: () {},
                ),
            ],
          ),
        ),
      ),
    );

    for (final persona in MkAgentPersona.values) {
      expect(find.text(persona.label), findsOneWidget);
    }
    expect(
      find.text('Draft only. AI cannot approve, send, or publish.'),
      findsNWidgets(4),
    );
    expect(
      find.text('Informational only. It cannot change state.'),
      findsOneWidget,
    );
  });

  testWidgets('renders stale result as non-reviewable provenance', (
    tester,
  ) async {
    final proposal = MkProposal(
      eventId: 'proposal-event',
      proposalId: 'proposal-id',
      targetId: 'person-id',
      targetKind: MkEntityType.person.kind,
      agent: 'Guest Researcher',
      summary: 'Historical research retained for audit.',
      provenance: const ['Synthetic guest profile'],
      clips: const [],
      createdAt: 1,
      schemaVersion: 2,
      personaId: MkAgentPersona.guestResearcher.id,
      targetEventId: 'old-event',
      targetVersion: 1,
      reviewState: 'stale',
    );
    await tester.pumpWidget(
      WidgetHelpers.testable(
        child: MkAgentPanel(
          persona: MkAgentPersona.guestResearcher,
          snapshot: MkIdeasSnapshot.fromRecords(proposals: [proposal]),
        ),
      ),
    );

    expect(
      find.text('STALE · A fresh run is required before review.'),
      findsOneWidget,
    );
    expect(find.text('STALE'), findsOneWidget);
  });
}
