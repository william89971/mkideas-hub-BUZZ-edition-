part of '../settings_page.dart';

class _DeviceSection extends StatelessWidget {
  const _DeviceSection();

  @override
  Widget build(BuildContext context) {
    return AppListCard(
      label: 'Security',
      verticalPadding: Grid.twelve,
      children: [
        AppListRow(
          icon: LucideIcons.shieldCheck,
          title: 'Devices & recovery',
          subtitle: 'Enroll, review, and revoke device access',
          trailing: const _RowChevron(),
          onTap: () => Navigator.of(context).push(
            MaterialPageRoute<void>(
              builder: (_) => const MkDeviceSecurityPage(),
            ),
          ),
        ),
      ],
    );
  }
}
