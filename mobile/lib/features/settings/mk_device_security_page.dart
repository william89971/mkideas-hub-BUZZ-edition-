import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../../shared/mkideas/mkideas_device_api.dart';
import '../../shared/relay/relay.dart';
import '../../shared/theme/theme.dart';
import '../../shared/widgets/app_list_card.dart';

final mkDeviceInventoryProvider =
    FutureProvider.autoDispose<List<MkDeviceGrant>>((ref) async {
      final api = MkIdeasDeviceApi(config: ref.watch(relayConfigProvider));
      ref.onDispose(api.close);
      return api.listDevices();
    });

class MkDeviceSecurityPage extends HookConsumerWidget {
  const MkDeviceSecurityPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final name = useTextEditingController(
      text: '${Platform.operatingSystem} device',
    );
    final working = useState(false);
    final inventory = ref.watch(mkDeviceInventoryProvider);

    Future<void> enroll() async {
      if (name.text.trim().isEmpty || working.value) return;
      working.value = true;
      final api = MkIdeasDeviceApi(config: ref.read(relayConfigProvider));
      try {
        await api.enroll(
          deviceName: name.text,
          platform: Platform.operatingSystem,
        );
        ref.invalidate(mkDeviceInventoryProvider);
        if (context.mounted) {
          ScaffoldMessenger.of(context).showSnackBar(
            const SnackBar(content: Text('This device is enrolled')),
          );
        }
      } catch (error) {
        if (context.mounted) {
          ScaffoldMessenger.of(context).showSnackBar(
            SnackBar(content: Text('Could not enroll device: $error')),
          );
        }
      } finally {
        api.close();
        working.value = false;
      }
    }

    return Scaffold(
      appBar: AppBar(title: const Text('Devices & recovery')),
      body: ListView(
        padding: const EdgeInsets.symmetric(vertical: Grid.sm),
        children: [
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: Grid.gutter),
            child: Text(
              'Each device gets an independent key in secure storage. Revoke one device without replacing your human identity.',
              style: context.textTheme.bodyMedium?.copyWith(
                color: context.colors.onSurfaceVariant,
              ),
            ),
          ),
          AppListCard(
            label: 'This device',
            verticalPadding: Grid.twelve,
            children: [
              Padding(
                padding: const EdgeInsets.all(Grid.xs),
                child: TextField(
                  controller: name,
                  maxLength: 100,
                  decoration: const InputDecoration(
                    labelText: 'Device name',
                    prefixIcon: Icon(LucideIcons.smartphone),
                  ),
                ),
              ),
              AppListRow(
                icon: LucideIcons.shieldCheck,
                title: working.value ? 'Enrolling…' : 'Enroll this device',
                subtitle: 'Requires both your identity and this device key',
                onTap: working.value ? null : enroll,
              ),
            ],
          ),
          AppListCard(
            label: 'Inventory',
            verticalPadding: Grid.twelve,
            children: inventory.when(
              loading: () => const [AppListRow(title: 'Loading devices…')],
              error: (error, _) => [
                AppListRow(
                  icon: LucideIcons.refreshCw,
                  title: 'Device service unavailable',
                  subtitle: '$error',
                  onTap: () => ref.invalidate(mkDeviceInventoryProvider),
                ),
              ],
              data: (devices) => devices.isEmpty
                  ? const [
                      AppListRow(
                        title: 'No enrolled devices',
                        subtitle:
                            'Enroll every team device before enabling enforcement',
                      ),
                    ]
                  : [
                      for (final device in devices)
                        AppListRow(
                          icon: device.revokedAt == null
                              ? LucideIcons.smartphone
                              : LucideIcons.ban,
                          title: device.deviceName,
                          subtitle: device.revokedAt == null
                              ? '${device.platform} · ${device.devicePubkey.substring(0, 12)}…'
                              : 'Revoked',
                          titleColor: device.revokedAt == null
                              ? null
                              : context.colors.onSurfaceVariant,
                          onTap: device.revokedAt == null
                              ? () => _confirmRevoke(context, ref, device)
                              : null,
                        ),
                    ],
            ),
          ),
        ],
      ),
    );
  }
}

Future<void> _confirmRevoke(
  BuildContext context,
  WidgetRef ref,
  MkDeviceGrant device,
) async {
  final confirmed = await showDialog<bool>(
    context: context,
    builder: (dialogContext) => AlertDialog(
      title: Text('Revoke ${device.deviceName}?'),
      content: const Text(
        'This immediately disconnects that device. It must be enrolled again before it can reconnect.',
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.pop(dialogContext, false),
          child: const Text('Cancel'),
        ),
        FilledButton(
          onPressed: () => Navigator.pop(dialogContext, true),
          child: const Text('Revoke'),
        ),
      ],
    ),
  );
  if (confirmed != true) return;
  final api = MkIdeasDeviceApi(config: ref.read(relayConfigProvider));
  try {
    await api.revoke(device.id, 'Device revoked from mobile settings');
    ref.invalidate(mkDeviceInventoryProvider);
  } catch (error) {
    if (context.mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Could not revoke device: $error')),
      );
    }
  } finally {
    api.close();
  }
}
