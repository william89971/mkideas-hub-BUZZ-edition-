import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../../shared/theme/theme.dart';

class MkWorkPage extends StatelessWidget {
  const MkWorkPage({required this.onSearch, super.key});
  final VoidCallback onSearch;
  @override
  Widget build(BuildContext context) => Scaffold(
    backgroundColor: context.colors.surface,
    body: SafeArea(
      child: Padding(
        padding: const EdgeInsets.fromLTRB(Grid.gutter, Grid.lg, Grid.gutter, 120),
        child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Row(children: [
            Expanded(child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
              Text('OPERATING RHYTHM', style: context.textTheme.labelSmall?.copyWith(color: context.colors.primary, letterSpacing: 1.6, fontWeight: FontWeight.w700)),
              Text('Work', style: context.textTheme.headlineLarge?.copyWith(fontFamily: 'Georgia', fontWeight: FontWeight.w700)),
            ])),
            IconButton(onPressed: onSearch, icon: const Icon(LucideIcons.search)),
          ]),
          const Spacer(),
          Center(child: Column(children: [
            const Icon(LucideIcons.inbox),
            const SizedBox(height: Grid.sm),
            Text('Deliberately quiet in V0', style: context.textTheme.titleMedium?.copyWith(fontWeight: FontWeight.w700)),
            const SizedBox(height: Grid.xs),
            Text('Goals, operational projects, tasks, meetings, and decisions arrive in V1.', textAlign: TextAlign.center, style: context.textTheme.bodyMedium?.copyWith(color: context.colors.onSurfaceVariant)),
          ])),
          const Spacer(),
        ]),
      ),
    ),
  );
}
