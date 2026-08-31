import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'mk_quick_capture_sheet.dart';

/// Keeps MK Quick Capture available beside the permanent five-area navigation.
class MkQuickCaptureLauncher extends ConsumerWidget {
  const MkQuickCaptureLauncher({
    required this.navigationBarHeight,
    required this.navigationBarBottomGap,
    required this.systemBottomInset,
    required this.leftInset,
    super.key,
  });

  final double navigationBarHeight;
  final double navigationBarBottomGap;
  final double systemBottomInset;
  final double leftInset;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final navigationBottomInset = systemBottomInset > navigationBarBottomGap
        ? systemBottomInset
        : navigationBarBottomGap;
    final bottom = navigationBottomInset + (navigationBarHeight - 56) / 2;
    return Stack(
      fit: StackFit.expand,
      children: [
        Positioned(
          left: leftInset,
          bottom: bottom,
          child: FloatingActionButton(
            key: const Key('mk-universal-quick-capture'),
            heroTag: 'mk-universal-quick-capture',
            onPressed: () {
              unawaited(HapticFeedback.lightImpact());
              unawaited(showMkQuickCaptureSheet(context));
            },
            tooltip: 'Quick Capture',
            child: const Icon(LucideIcons.plus),
          ),
        ),
      ],
    );
  }
}
