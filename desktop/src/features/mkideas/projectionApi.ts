import type { RelayEvent } from "@/shared/api/types";
import { invokeTauri } from "@/shared/api/tauri";

export type MkProjection = "heads" | "history";

export type MkProjectionQuery = {
  projection: MkProjection;
  kinds: number[];
  community: string;
  entityId?: string;
  cursor?: string;
  limit?: number;
};

export type MkProjectionPage = {
  events: RelayEvent[];
  nextCursor: string | null;
};

type RawMkProjectionPage = {
  events: RelayEvent[];
  nextCursor?: string | null;
  next_cursor?: string | null;
};

export type MkProjectionTransport = (
  query: MkProjectionQuery,
) => Promise<MkProjectionPage>;

export function normalizeMkProjectionPage(
  page: RawMkProjectionPage,
): MkProjectionPage {
  if (!Array.isArray(page.events)) {
    throw new Error("MK Ideas projection response did not include events.");
  }
  const nextCursor = page.nextCursor ?? page.next_cursor ?? null;
  if (nextCursor !== null && typeof nextCursor !== "string") {
    throw new Error("MK Ideas projection returned an invalid cursor.");
  }
  return { events: page.events, nextCursor };
}

export const queryMkProjection: MkProjectionTransport = async (query) => {
  const response = await invokeTauri<RawMkProjectionPage>(
    "query_mkideas_projection",
    { input: query },
  );
  return normalizeMkProjectionPage(response);
};

export async function collectMkProjection(
  query: Omit<MkProjectionQuery, "cursor">,
  transport: MkProjectionTransport = queryMkProjection,
): Promise<RelayEvent[]> {
  const events: RelayEvent[] = [];
  const seenCursors = new Set<string>();
  let cursor: string | undefined;
  do {
    const page = await transport({ ...query, cursor });
    events.push(...page.events);
    if (!page.nextCursor) return events;
    if (seenCursors.has(page.nextCursor)) {
      throw new Error("MK Ideas projection returned a repeated cursor.");
    }
    seenCursors.add(page.nextCursor);
    cursor = page.nextCursor;
  } while (cursor);
  return events;
}
