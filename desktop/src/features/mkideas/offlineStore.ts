import type { RelayEvent } from "@/shared/api/types";

import { publishSignedMkState, signMkState, type MkStateInput } from "./api";
import {
  parseMkIdeasEvents,
  type MkIdeasSnapshot,
  type MkRecord,
} from "./model";

const STORE_VERSION = 1;
const OUTBOX_EVENT = "mkideas-outbox-changed";

type StorageLike = Pick<Storage, "getItem" | "setItem" | "removeItem">;

export type MkOutboxStatus = "queued" | "conflict";

export type StoredMkStateInput = Omit<MkStateInput, "previous"> & {
  previous?: MkRecord;
};

export type MkOutboxItem = {
  id: string;
  relayUrl: string;
  event: RelayEvent;
  input: StoredMkStateInput;
  status: MkOutboxStatus;
  attempts: number;
  createdAt: string;
  updatedAt: string;
  error?: string;
};

type PersistedOutbox = {
  version: typeof STORE_VERSION;
  items: MkOutboxItem[];
};

type PersistedCache = {
  version: typeof STORE_VERSION;
  savedAt: string;
  events: RelayEvent[];
};

function storageOrNull(storage?: StorageLike): StorageLike | null {
  if (storage) return storage;
  if (typeof window === "undefined") return null;
  return window.localStorage;
}

function communityKey(relayUrl: string): string {
  return encodeURIComponent(relayUrl.trim().toLowerCase());
}

function outboxKey(relayUrl: string): string {
  return `buzz.mkideas.outbox.v1:${communityKey(relayUrl)}`;
}

function cacheKey(relayUrl: string): string {
  return `buzz.mkideas.cache.v1:${communityKey(relayUrl)}`;
}

function emitOutboxChanged(): void {
  if (typeof window !== "undefined") {
    window.dispatchEvent(new Event(OUTBOX_EVENT));
  }
}

export function isMkConflictError(error: unknown): boolean {
  const message = error instanceof Error ? error.message : String(error);
  return /\bconflict\b|stale|superseded|version/i.test(message);
}

export function readMkOutbox(
  relayUrl: string,
  storage?: StorageLike,
  pubkey?: string,
): MkOutboxItem[] {
  const target = storageOrNull(storage);
  if (!target) return [];
  try {
    const raw = target.getItem(outboxKey(relayUrl));
    if (!raw) return [];
    const parsed = JSON.parse(raw) as Partial<PersistedOutbox>;
    if (parsed.version !== STORE_VERSION || !Array.isArray(parsed.items)) {
      return [];
    }
    return parsed.items.filter(
      (item): item is MkOutboxItem =>
        Boolean(item) &&
        typeof item.id === "string" &&
        typeof item.relayUrl === "string" &&
        (item.status === "queued" || item.status === "conflict") &&
        typeof item.event === "object" &&
        typeof item.input === "object" &&
        (!pubkey || item.event.pubkey.toLowerCase() === pubkey.toLowerCase()),
    );
  } catch {
    return [];
  }
}

function writeMkOutbox(
  relayUrl: string,
  items: MkOutboxItem[],
  storage?: StorageLike,
): void {
  const target = storageOrNull(storage);
  if (!target) return;
  if (items.length === 0) target.removeItem(outboxKey(relayUrl));
  else {
    target.setItem(
      outboxKey(relayUrl),
      JSON.stringify({
        version: STORE_VERSION,
        items,
      } satisfies PersistedOutbox),
    );
  }
  emitOutboxChanged();
}

export function cacheMkIdeasEvents(
  relayUrl: string,
  events: RelayEvent[],
  storage?: StorageLike,
): void {
  const target = storageOrNull(storage);
  if (!target) return;
  target.setItem(
    cacheKey(relayUrl),
    JSON.stringify({
      version: STORE_VERSION,
      savedAt: new Date().toISOString(),
      events,
    } satisfies PersistedCache),
  );
}

export function readCachedMkIdeasSnapshot(
  relayUrl: string,
  storage?: StorageLike,
): { snapshot: MkIdeasSnapshot; savedAt: string } | null {
  const target = storageOrNull(storage);
  if (!target) return null;
  try {
    const raw = target.getItem(cacheKey(relayUrl));
    if (!raw) return null;
    const parsed = JSON.parse(raw) as Partial<PersistedCache>;
    if (
      parsed.version !== STORE_VERSION ||
      typeof parsed.savedAt !== "string" ||
      !Array.isArray(parsed.events)
    ) {
      return null;
    }
    return {
      snapshot: parseMkIdeasEvents(parsed.events),
      savedAt: parsed.savedAt,
    };
  } catch {
    return null;
  }
}

export function storeMkOutboxItem(
  item: MkOutboxItem,
  storage?: StorageLike,
): void {
  const items = readMkOutbox(item.relayUrl, storage);
  const index = items.findIndex((candidate) => candidate.id === item.id);
  if (index === -1) items.push(item);
  else items[index] = item;
  writeMkOutbox(item.relayUrl, items, storage);
}

export function discardMkOutboxItem(
  relayUrl: string,
  itemId: string,
  storage?: StorageLike,
): void {
  writeMkOutbox(
    relayUrl,
    readMkOutbox(relayUrl, storage).filter((item) => item.id !== itemId),
    storage,
  );
}

export async function publishOrQueueMkState(
  relayUrl: string,
  input: MkStateInput,
  storage?: StorageLike,
): Promise<{ event: RelayEvent; status: "published" | MkOutboxStatus }> {
  const event = await signMkState(relayUrl, input);
  const now = new Date().toISOString();
  const item: MkOutboxItem = {
    id: event.id,
    relayUrl,
    event,
    input,
    status: "queued",
    attempts: 0,
    createdAt: now,
    updatedAt: now,
  };
  storeMkOutboxItem(item, storage);
  try {
    await publishSignedMkState(event);
    discardMkOutboxItem(relayUrl, item.id, storage);
    return { event, status: "published" };
  } catch (error) {
    const status = isMkConflictError(error) ? "conflict" : "queued";
    storeMkOutboxItem(
      {
        ...item,
        status,
        attempts: 1,
        updatedAt: new Date().toISOString(),
        error: error instanceof Error ? error.message : String(error),
      },
      storage,
    );
    return { event, status };
  }
}

export async function flushMkOutbox(
  relayUrl: string,
  storage?: StorageLike,
  pubkey?: string,
): Promise<MkOutboxItem[]> {
  const items = readMkOutbox(relayUrl, storage, pubkey);
  for (const item of items) {
    if (item.status === "conflict") continue;
    try {
      await publishSignedMkState(item.event);
      discardMkOutboxItem(relayUrl, item.id, storage);
    } catch (error) {
      storeMkOutboxItem(
        {
          ...item,
          status: isMkConflictError(error) ? "conflict" : "queued",
          attempts: item.attempts + 1,
          updatedAt: new Date().toISOString(),
          error: error instanceof Error ? error.message : String(error),
        },
        storage,
      );
      if (!isMkConflictError(error)) break;
    }
  }
  return readMkOutbox(relayUrl, storage, pubkey);
}

export async function reapplyMkOutboxItem(
  relayUrl: string,
  item: MkOutboxItem,
  latestRecords: MkRecord[],
  storage?: StorageLike,
): Promise<"published" | MkOutboxStatus> {
  const latest = latestRecords.find(
    (record) =>
      record.kind === item.input.kind &&
      record.entityId ===
        (item.input.entityId ?? item.input.previous?.entityId ?? ""),
  );
  try {
    const result = await publishOrQueueMkState(
      relayUrl,
      { ...item.input, previous: latest },
      storage,
    );
    discardMkOutboxItem(relayUrl, item.id, storage);
    return result.status;
  } catch (error) {
    // Signing failures must not erase the human's original conflicted draft.
    storeMkOutboxItem(item, storage);
    throw error;
  }
}

export function subscribeMkOutbox(listener: () => void): () => void {
  if (typeof window === "undefined") return () => undefined;
  window.addEventListener(OUTBOX_EVENT, listener);
  window.addEventListener("storage", listener);
  return () => {
    window.removeEventListener(OUTBOX_EVENT, listener);
    window.removeEventListener("storage", listener);
  };
}
