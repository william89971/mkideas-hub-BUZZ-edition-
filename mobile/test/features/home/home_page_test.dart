import 'package:buzz/features/home/home_page.dart';
import 'package:buzz/features/mkideas/mk_quick_capture_launcher.dart';
import 'package:buzz/features/profile/profile_avatar.dart';
import 'package:buzz/shared/theme/theme.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:shared_preferences/shared_preferences.dart';

void main() {
  Future<Widget> buildHome({
    int unreadInboxCount = 0,
    bool disableAnimations = false,
    Gradient? topSectionGradient,
  }) async {
    SharedPreferences.setMockInitialValues({});
    final prefs = await SharedPreferences.getInstance();
    return ProviderScope(
      overrides: [savedPrefsProvider.overrideWithValue(prefs)],
      child: MaterialApp(
        theme: AppTheme.light(topSectionGradient: topSectionGradient),
        builder: (context, child) => MediaQuery(
          data: MediaQuery.of(
            context,
          ).copyWith(disableAnimations: disableAnimations),
          child: child!,
        ),
        home: HomePage(
          settingsPageBuilder: _buildSettingsPage,
          hasUnreadInbox: unreadInboxCount > 0,
        ),
      ),
    );
  }

  testWidgets('shows the five-area navigation and aligned Quick Capture', (
    tester,
  ) async {
    await tester.pumpWidget(await buildHome());
    await tester.pump();

    expect(find.byTooltip('Today'), findsOneWidget);
    expect(find.byTooltip('Work'), findsOneWidget);
    expect(find.byTooltip('People'), findsOneWidget);
    expect(find.byTooltip('Studio'), findsOneWidget);
    expect(find.byTooltip('Team'), findsOneWidget);

    final quickAction = find.byTooltip('Quick Capture');
    expect(quickAction, findsOneWidget);
    final launcherSize = tester.getSize(find.byType(MkQuickCaptureLauncher));
    expect(launcherSize.width, 800);
    expect(launcherSize.height, greaterThan(0));
    expect(tester.getSize(quickAction), const Size.square(56));
    final quickActionRect = tester.getRect(quickAction);
    expect(quickActionRect.left, greaterThanOrEqualTo(0));
    expect(quickActionRect.top, greaterThanOrEqualTo(0));
    expect(quickActionRect.right, lessThanOrEqualTo(800));
    expect(quickActionRect.bottom, lessThanOrEqualTo(600));
    final homeDestinationRect = tester.getRect(find.byTooltip('Today'));
    expect(
      quickActionRect.center.dy,
      closeTo(homeDestinationRect.center.dy, 0.01),
    );
  });

  testWidgets('keeps the Buzz backdrop behind the scalable Team screen', (
    tester,
  ) async {
    const gradient = LinearGradient(
      begin: Alignment.topCenter,
      end: Alignment.bottomCenter,
      colors: [Colors.yellow, Colors.blue],
    );
    await tester.pumpWidget(await buildHome(topSectionGradient: gradient));
    await tester.pumpAndSettle();
    await tester.tap(find.byTooltip('Team'));
    await tester.pumpAndSettle();

    final backdrop = find.byKey(
      const ValueKey('home-settings-transition-backdrop'),
    );
    final decoration =
        tester.widget<DecoratedBox>(backdrop).decoration as BoxDecoration;
    expect(decoration.gradient, gradient);
    expect(
      find.byKey(const ValueKey('home-settings-transition-scale')),
      findsOneWidget,
    );
    expect(
      tester
          .widget<Transform>(
            find.byKey(const ValueKey('home-settings-transition-scale')),
          )
          .transform
          .getMaxScaleOnAxis(),
      1,
    );
    expect(
      tester
          .widget<Opacity>(
            find.byKey(const ValueKey('home-settings-transition-opacity')),
          )
          .opacity,
      1,
    );
  });

  testWidgets('keeps Home opaque beneath the Settings transition', (
    tester,
  ) async {
    await tester.pumpWidget(await buildHome());
    await tester.pumpAndSettle();

    await tester.tap(find.byTooltip('Team'));
    await tester.pumpAndSettle();
    await tester.tap(find.byType(ProfileAvatar));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 95));

    double homeOpacity() => tester
        .widget<Opacity>(
          find.byKey(const ValueKey('home-settings-transition-opacity')),
        )
        .opacity;

    expect(homeOpacity(), 1);

    await tester.pumpAndSettle();
    Navigator.of(
      tester.element(
        find.byKey(
          const ValueKey('settings-transition-opacity'),
          skipOffstage: false,
        ),
      ),
    ).pop();
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 95));

    expect(homeOpacity(), 1);
  });

  testWidgets('uses one monotonic route animation for Settings and Home', (
    tester,
  ) async {
    await tester.pumpWidget(await buildHome());
    await tester.pumpAndSettle();

    double homeScale() => tester
        .widget<Transform>(
          find.byKey(const ValueKey('home-settings-transition-scale')),
        )
        .transform
        .storage[0];

    await tester.tap(find.byTooltip('Team'));
    await tester.pumpAndSettle();
    await tester.tap(find.byType(ProfileAvatar));
    await tester.pump();

    final settingsTransition = find.byKey(
      const ValueKey('settings-transition-opacity'),
      skipOffstage: false,
    );
    final settingsRoute = ModalRoute.of(tester.element(settingsTransition));

    final entranceScales = <double>[homeScale()];
    final routeValues = <double>[settingsRoute!.animation!.value];
    for (var frame = 0; frame < 15; frame++) {
      await tester.pump(const Duration(milliseconds: 16));
      entranceScales.add(homeScale());
      routeValues.add(settingsRoute.animation!.value);
    }
    expect(entranceScales.first, closeTo(1, 0.000001));
    final reversalFrames = <int>[];
    for (var frame = 1; frame < entranceScales.length; frame++) {
      if (entranceScales[frame] > entranceScales[frame - 1] + 0.000001) {
        reversalFrames.add(frame);
      }
    }
    expect(
      reversalFrames,
      isEmpty,
      reason:
          'Home must scale down in one direction on entrance. '
          'scales=$entranceScales route=$routeValues',
    );
    expect(entranceScales, everyElement(inInclusiveRange(0.97, 1)));
    expect(entranceScales.last, closeTo(0.97, 0.001));

    await tester.pumpAndSettle();
    Navigator.of(tester.element(settingsTransition)).pop();
    await tester.pumpAndSettle();
  });

  testWidgets('gives selection haptics only when the tab changes', (
    tester,
  ) async {
    final hapticCalls = <MethodCall>[];
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(SystemChannels.platform, (call) async {
          if (call.method == 'HapticFeedback.vibrate') {
            hapticCalls.add(call);
          }
          return null;
        });
    addTearDown(
      () => TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(SystemChannels.platform, null),
    );

    await tester.pumpWidget(await buildHome());
    await tester.pump();

    await tester.tap(find.byTooltip('Today'));
    await tester.pump();
    expect(hapticCalls, isEmpty);

    await tester.tap(find.byTooltip('Work'));
    await tester.pump();
    expect(hapticCalls, hasLength(1));
    expect(hapticCalls.single.arguments, 'HapticFeedbackType.selectionClick');

    await tester.tap(find.byTooltip('Work'));
    await tester.pump();
    expect(hapticCalls, hasLength(1));

    await tester.tap(find.byTooltip('People'));
    await tester.pump();
    expect(hapticCalls, hasLength(2));
  });

  testWidgets('gives a light impact when universal Quick Capture is pressed', (
    tester,
  ) async {
    final hapticCalls = <MethodCall>[];
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(SystemChannels.platform, (call) async {
          if (call.method == 'HapticFeedback.vibrate') {
            hapticCalls.add(call);
          }
          return null;
        });
    addTearDown(
      () => TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(SystemChannels.platform, null),
    );

    await tester.pumpWidget(await buildHome());
    await tester.pump();

    await tester.tap(find.byTooltip('Quick Capture'));
    await tester.pump();

    expect(hapticCalls, hasLength(1));
    expect(hapticCalls.single.arguments, 'HapticFeedbackType.lightImpact');
  });

  testWidgets('badges the Team tab when it has unread rows', (tester) async {
    await tester.pumpWidget(await buildHome(unreadInboxCount: 1));
    await tester.pump();

    expect(
      find.byKey(const ValueKey('activity-tab-unread-dot')),
      findsOneWidget,
    );
    final badge = tester.widget<Container>(
      find.byKey(const ValueKey('activity-tab-unread-dot')),
    );
    expect(badge.constraints?.maxWidth, 12);
    expect(badge.constraints?.maxHeight, 12);
    expect(find.byTooltip('Team'), findsOneWidget);
    AnimatedScale unreadDotScale() => tester.widget<AnimatedScale>(
      find.byKey(const ValueKey('activity-tab-unread-dot-scale')),
    );
    expect(unreadDotScale().scale, 1);
    expect(unreadDotScale().alignment, const Alignment(-0.5, 0.5));
    expect(unreadDotScale().duration, const Duration(milliseconds: 220));

    await tester.tap(find.byTooltip('Team'));
    await tester.pump();

    expect(
      find.byKey(const ValueKey('activity-tab-unread-dot')),
      findsOneWidget,
    );
    expect(unreadDotScale().scale, 0);
    expect(find.byTooltip('Team'), findsOneWidget);

    await tester.tap(find.byTooltip('Today'));
    await tester.pump();

    expect(unreadDotScale().scale, 1);
  });

  testWidgets('fades and slides tab content in the selected direction', (
    tester,
  ) async {
    await tester.pumpWidget(await buildHome());
    await tester.pump();

    Transform areaTransform() => tester.widget<Transform>(
      find.byKey(const ValueKey('mk-area-tab-transition-transform')),
    );
    Opacity areaOpacity() => tester.widget<Opacity>(
      find.byKey(const ValueKey('mk-area-tab-transition-opacity')),
    );
    double areaOffset() => areaTransform().transform.getTranslation().x;

    expect(areaOffset(), closeTo(0, 0.001));
    expect(areaOpacity().opacity, closeTo(1, 0.001));

    await tester.tap(find.byTooltip('Work'));
    await tester.pump();

    expect(areaOffset(), closeTo(24, 0.001));
    expect(areaOpacity().opacity, closeTo(0, 0.001));

    await tester.pump(const Duration(milliseconds: 120));

    expect(areaOffset(), inExclusiveRange(0, 24));
    expect(areaOpacity().opacity, inExclusiveRange(0, 1));

    await tester.pumpAndSettle();
    await tester.tap(find.byTooltip('Today'));
    await tester.pump();

    expect(areaOffset(), closeTo(-24, 0.001));
    expect(areaOpacity().opacity, closeTo(0, 0.001));

    await tester.pumpAndSettle();
    expect(areaOffset(), closeTo(0, 0.001));
    expect(areaOpacity().opacity, closeTo(1, 0.001));
  });

  testWidgets('switches tab content instantly with reduced motion', (
    tester,
  ) async {
    await tester.pumpWidget(await buildHome(disableAnimations: true));
    await tester.pump();

    await tester.tap(find.byTooltip('Work'));
    await tester.pump();

    final areaTransform = tester.widget<Transform>(
      find.byKey(const ValueKey('mk-area-tab-transition-transform')),
    );
    final areaOpacity = tester.widget<Opacity>(
      find.byKey(const ValueKey('mk-area-tab-transition-opacity')),
    );
    expect(areaTransform.transform.getTranslation().x, closeTo(0, 0.001));
    expect(areaOpacity.opacity, closeTo(1, 0.001));
  });

  testWidgets('scales and fades Team conversation actions as tabs change', (
    tester,
  ) async {
    await tester.pumpWidget(await buildHome());
    await tester.pump();

    double scale() => tester
        .widget<Transform>(find.byKey(const Key('channel-quick-actions-scale')))
        .transform
        .storage
        .first;
    double opacity() => tester
        .widget<Opacity>(find.byKey(const Key('channel-quick-actions-opacity')))
        .opacity;

    expect(scale(), closeTo(0.8, 0.001));
    expect(opacity(), closeTo(0, 0.001));

    await tester.tap(find.byTooltip('Team'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 110));

    expect(scale(), inExclusiveRange(0.8, 1));
    expect(opacity(), inExclusiveRange(0, 1));

    await tester.pumpAndSettle();
    expect(scale(), closeTo(1, 0.001));
    expect(opacity(), closeTo(1, 0.001));

    await tester.tap(find.byTooltip('Today'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 110));

    expect(scale(), inExclusiveRange(0.8, 1));
    expect(opacity(), inExclusiveRange(0, 1));

    await tester.pumpAndSettle();
    expect(scale(), closeTo(0.8, 0.001));
    expect(opacity(), closeTo(0, 0.001));
  });
}

Widget _buildSettingsPage(BuildContext context) => const SizedBox.shrink();
