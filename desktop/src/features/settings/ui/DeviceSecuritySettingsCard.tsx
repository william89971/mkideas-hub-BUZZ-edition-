import * as React from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { KeyRound, Laptop, RefreshCw, ShieldCheck, Trash2 } from "lucide-react";
import { toast } from "sonner";

import {
  enrollThisDevice,
  listMkDevices,
  revokeMkDevice,
} from "@/features/mkideas/deviceApi";
import { Button } from "@/shared/ui/button";
import {
  SettingsOptionGroup,
  SettingsOptionGroupList,
  SettingsOptionRow,
} from "./SettingsOptionGroup";
import { SettingsSectionHeader } from "./SettingsSectionHeader";

function navigatorPlatform(): string {
  const navigatorWithClientHints = navigator as Navigator & {
    userAgentData?: { platform?: string };
  };
  return navigatorWithClientHints.userAgentData?.platform ?? navigator.platform;
}

function suggestedDeviceName(): string {
  const platform = navigatorPlatform();
  return platform ? `${platform} computer` : "My computer";
}

function platformLabel(): string {
  const value = navigatorPlatform().toLowerCase();
  if (value.includes("mac")) return "macos";
  if (value.includes("win")) return "windows";
  if (value.includes("linux")) return "linux";
  return "desktop";
}

function dateLabel(value: string | null): string {
  return value ? new Date(value).toLocaleString() : "Never";
}

export function DeviceSecuritySettingsCard() {
  const queryClient = useQueryClient();
  const [deviceName, setDeviceName] = React.useState(suggestedDeviceName);
  const devicesQuery = useQuery({
    queryKey: ["mkideas", "devices"],
    queryFn: listMkDevices,
    retry: false,
  });
  const enroll = useMutation({
    mutationFn: () =>
      enrollThisDevice({
        deviceName: deviceName.trim(),
        platform: platformLabel(),
      }),
    onSuccess: async () => {
      toast.success("This device is enrolled");
      await queryClient.invalidateQueries({ queryKey: ["mkideas", "devices"] });
    },
    onError: (error) => toast.error(error.message),
  });
  const revoke = useMutation({
    mutationFn: ({ id, reason }: { id: string; reason: string }) =>
      revokeMkDevice(id, reason),
    onSuccess: async () => {
      toast.success("Device access revoked");
      await queryClient.invalidateQueries({ queryKey: ["mkideas", "devices"] });
    },
    onError: (error) => toast.error(error.message),
  });
  const devices = devicesQuery.data ?? [];

  return (
    <section className="space-y-5" data-testid="device-security-settings">
      <SettingsSectionHeader
        description="Give each computer its own revocable key. Your human identity key never leaves secure storage."
        title="Devices & recovery"
      />
      <SettingsOptionGroupList>
        <SettingsOptionGroup title="This computer">
          <SettingsOptionRow>
            <div className="flex min-w-0 flex-1 items-center gap-3">
              <ShieldCheck className="h-5 w-5 shrink-0 text-emerald-600" />
              <div className="min-w-0">
                <p className="text-sm font-medium">Independent device key</p>
                <p className="text-xs text-muted-foreground">
                  Stored in the operating system keyring and proven together
                  with your human identity.
                </p>
              </div>
            </div>
          </SettingsOptionRow>
          <SettingsOptionRow>
            <div className="flex min-w-0 flex-1 items-center gap-3">
              <KeyRound className="h-5 w-5 shrink-0 text-muted-foreground" />
              <input
                aria-label="Device name"
                className="min-w-0 flex-1 rounded-md border bg-background px-3 py-2 text-sm"
                maxLength={100}
                onChange={(event) => setDeviceName(event.target.value)}
                value={deviceName}
              />
            </div>
            <Button
              disabled={!deviceName.trim() || enroll.isPending}
              onClick={() => enroll.mutate()}
              size="sm"
            >
              {enroll.isPending ? "Enrolling…" : "Enroll this device"}
            </Button>
          </SettingsOptionRow>
        </SettingsOptionGroup>

        <SettingsOptionGroup title="Device inventory">
          {devicesQuery.isLoading ? (
            <SettingsOptionRow>
              <p className="text-sm text-muted-foreground">Loading devices…</p>
            </SettingsOptionRow>
          ) : null}
          {devicesQuery.error ? (
            <SettingsOptionRow>
              <div className="min-w-0 flex-1">
                <p className="text-sm font-medium">
                  Device service unavailable
                </p>
                <p className="text-xs text-muted-foreground">
                  {devicesQuery.error.message}
                </p>
              </div>
              <Button
                onClick={() => void devicesQuery.refetch()}
                size="sm"
                variant="outline"
              >
                <RefreshCw className="h-4 w-4" /> Retry
              </Button>
            </SettingsOptionRow>
          ) : null}
          {!devicesQuery.isLoading &&
          !devicesQuery.error &&
          devices.length === 0 ? (
            <SettingsOptionRow>
              <p className="text-sm text-muted-foreground">
                No devices are enrolled yet. Enroll every team device before
                enabling enforcement.
              </p>
            </SettingsOptionRow>
          ) : null}
          {devices.map((device) => (
            <SettingsOptionRow key={device.id}>
              <div className="flex min-w-0 flex-1 items-center gap-3">
                <Laptop className="h-5 w-5 shrink-0 text-muted-foreground" />
                <div className="min-w-0">
                  <p className="truncate text-sm font-medium">
                    {device.deviceName}
                    {device.revokedAt ? " · Revoked" : ""}
                  </p>
                  <p className="truncate text-xs text-muted-foreground">
                    {device.platform} · last used {dateLabel(device.lastSeenAt)}{" "}
                    · {device.devicePubkey.slice(0, 12)}…
                  </p>
                </div>
              </div>
              {!device.revokedAt ? (
                <Button
                  disabled={revoke.isPending}
                  onClick={() => {
                    const reason = window.prompt(
                      `Why are you revoking ${device.deviceName}?`,
                      "Device replaced or no longer trusted",
                    );
                    if (!reason) return;
                    if (reason.trim().length < 8) {
                      toast.error(
                        "Please give a reason of at least 8 characters",
                      );
                      return;
                    }
                    revoke.mutate({ id: device.id, reason: reason.trim() });
                  }}
                  size="sm"
                  variant="outline"
                >
                  <Trash2 className="h-4 w-4" /> Revoke
                </Button>
              ) : null}
            </SettingsOptionRow>
          ))}
        </SettingsOptionGroup>
      </SettingsOptionGroupList>
    </section>
  );
}
