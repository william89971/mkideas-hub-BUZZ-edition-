import { getIdentity } from "@/shared/api/tauriIdentity";
import {
  ensureMkIdeasDeviceIdentity,
  getRelayHttpUrl,
  rememberMkIdeasDeviceGrant,
  signMkIdeasDeviceEvent,
  signRelayEvent,
} from "@/shared/api/tauri";

const NIP98_KIND = 27235;
const AUTH_KIND = 22242;
const REQUEST_TIMEOUT_MS = 15_000;

export type MkDeviceGrant = {
  id: string;
  humanPubkey: string;
  devicePubkey: string;
  deviceName: string;
  platform: string;
  enrollmentMethod: string;
  issuedAt: string;
  expiresAt: string | null;
  lastSeenAt: string | null;
  revokedAt: string | null;
  revocationReason: string | null;
  authEpoch: number;
};

type RawDeviceGrant = {
  id: string;
  human_pubkey: string;
  device_pubkey: string;
  device_name: string;
  platform: string;
  enrollment_method: string;
  issued_at: string;
  expires_at: string | null;
  last_seen_at: string | null;
  revoked_at: string | null;
  revocation_reason: string | null;
  auth_epoch: number;
};

async function sha256Hex(text: string): Promise<string> {
  const digest = await crypto.subtle.digest(
    "SHA-256",
    new TextEncoder().encode(text),
  );
  return Array.from(new Uint8Array(digest), (byte) =>
    byte.toString(16).padStart(2, "0"),
  ).join("");
}

async function nip98Header(
  url: string,
  method: "GET" | "POST",
  body?: string,
): Promise<string> {
  const event = await signRelayEvent({
    kind: NIP98_KIND,
    content: "",
    tags: [
      ["u", url],
      ["method", method],
      ...(body ? [["payload", await sha256Hex(body)]] : []),
      ["nonce", crypto.randomUUID()],
    ],
  });
  return `Nostr ${btoa(JSON.stringify(event))}`;
}

async function request<T>(
  path: string,
  options: { method: "GET" | "POST"; body?: unknown },
): Promise<T> {
  const base = (await getRelayHttpUrl()).replace(/\/+$/, "");
  const url = `${base}${path}`;
  const body =
    options.body === undefined ? undefined : JSON.stringify(options.body);
  const response = await fetch(url, {
    method: options.method,
    headers: {
      Authorization: await nip98Header(url, options.method, body),
      ...(body ? { "Content-Type": "application/json" } : {}),
    },
    body,
    signal: AbortSignal.timeout(REQUEST_TIMEOUT_MS),
  });
  const json = (await response.json().catch(() => ({}))) as Record<
    string,
    unknown
  >;
  if (!response.ok) {
    throw new Error(
      typeof json.error === "string" ? json.error : `HTTP ${response.status}`,
    );
  }
  return json as T;
}

export async function listMkDevices(): Promise<MkDeviceGrant[]> {
  const result = await request<{ devices: RawDeviceGrant[] }>("/api/devices", {
    method: "GET",
  });
  return result.devices.map((device) => ({
    id: device.id,
    humanPubkey: device.human_pubkey,
    devicePubkey: device.device_pubkey,
    deviceName: device.device_name,
    platform: device.platform,
    enrollmentMethod: device.enrollment_method,
    issuedAt: device.issued_at,
    expiresAt: device.expires_at,
    lastSeenAt: device.last_seen_at,
    revokedAt: device.revoked_at,
    revocationReason: device.revocation_reason,
    authEpoch: device.auth_epoch,
  }));
}

export async function enrollThisDevice(input: {
  deviceName: string;
  platform: string;
}): Promise<string> {
  const [identity, base] = await Promise.all([
    getIdentity(),
    getRelayHttpUrl(),
  ]);
  const scope = `${identity.pubkey}:${base.toLowerCase()}`;
  const devicePubkey = await ensureMkIdeasDeviceIdentity(scope);
  const challenge = await request<{
    challenge_id: string;
    challenge: string;
    community_id: string;
    human_pubkey: string;
    device_pubkey: string;
    relay_url: string;
  }>("/api/devices/enrollment-challenges", {
    method: "POST",
    body: { device_pubkey: devicePubkey },
  });
  const proof = await signMkIdeasDeviceEvent({
    scope,
    kind: AUTH_KIND,
    content: JSON.stringify({
      version: 1,
      action: "enroll",
      community_id: challenge.community_id,
      human_pubkey: challenge.human_pubkey,
      device_pubkey: challenge.device_pubkey,
      challenge_id: challenge.challenge_id,
      challenge_hash: await sha256Hex(challenge.challenge),
      relay_url: challenge.relay_url,
    }),
  });
  const result = await request<{ grant_id: string }>("/api/devices/enroll", {
    method: "POST",
    body: {
      challenge_id: challenge.challenge_id,
      challenge: challenge.challenge,
      device_name: input.deviceName,
      platform: input.platform,
      device_proof: proof,
    },
  });
  await rememberMkIdeasDeviceGrant({
    scope,
    communityId: challenge.community_id,
    grantId: result.grant_id,
    relayUrl: challenge.relay_url,
  });
  return result.grant_id;
}

export async function revokeMkDevice(
  grantId: string,
  reason: string,
): Promise<void> {
  await request("/api/devices/revoke", {
    method: "POST",
    body: { grant_id: grantId, reason },
  });
}
