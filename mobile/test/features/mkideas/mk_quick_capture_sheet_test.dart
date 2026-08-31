import 'package:buzz/features/mkideas/mk_quick_capture_sheet.dart';
import 'package:buzz/shared/mkideas/mkideas_provider.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import '../../helpers/widget_helpers.dart';

void main() {
  testWidgets('captures a guest with MK-specific context', (tester) async {
    final notifier = _FakeMkIdeasNotifier(MkIdeasSnapshot.empty);
    await tester.pumpWidget(
      WidgetHelpers.testable(
        overrides: [mkIdeasProvider.overrideWith(() => notifier)],
        child: Builder(
          builder: (context) => Center(
            child: FilledButton(
              onPressed: () => showMkQuickCaptureSheet(
                context,
                initialType: MkCaptureType.guest,
              ),
              child: const Text('Open capture'),
            ),
          ),
        ),
      ),
    );

    await tester.tap(find.text('Open capture'));
    await tester.pumpAndSettle();
    await tester.enterText(
      find.byKey(const Key('mk-quick-capture-title')),
      'Avery Stone',
    );
    await tester.enterText(
      find.byKey(const Key('mk-quick-capture-organization')),
      'Signal House',
    );
    await tester.enterText(
      find.byKey(const Key('mk-quick-capture-why-now')),
      'New documentary release',
    );
    await tester.tap(find.byKey(const Key('mk-quick-capture-save')));
    await tester.pumpAndSettle();

    expect(notifier.captured, hasLength(1));
    expect(notifier.captured.single.type, MkCaptureType.guest);
    expect(notifier.captured.single.title, 'Avery Stone');
    expect(notifier.captured.single.organization, 'Signal House');
    expect(notifier.captured.single.whyNow, 'New documentary release');
  });

  testWidgets('keeps the sheet open until a title is supplied', (tester) async {
    final notifier = _FakeMkIdeasNotifier(MkIdeasSnapshot.empty);
    await tester.pumpWidget(
      WidgetHelpers.testable(
        overrides: [mkIdeasProvider.overrideWith(() => notifier)],
        child: const MkQuickCaptureSheet(),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.byKey(const Key('mk-quick-capture-save')));
    await tester.pump();

    expect(find.text('Add a title before saving.'), findsOneWidget);
    expect(notifier.captured, isEmpty);
  });
}

class _FakeMkIdeasNotifier extends MkIdeasNotifier {
  _FakeMkIdeasNotifier(this.snapshot);

  final MkIdeasSnapshot snapshot;
  final List<MkCaptureDraft> captured = [];

  @override
  Future<MkIdeasSnapshot> build() async => snapshot;

  @override
  Future<void> capture(MkCaptureDraft draft) async {
    captured.add(draft);
  }
}
