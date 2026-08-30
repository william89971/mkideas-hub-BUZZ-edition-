import type { RelayEvent } from "@/shared/api/types";

export type MkRecordType = "person" | "interview" | "content" | "approval";

export type MkRecord = {
  eventId: string;
  kind: number;
  author: string;
  createdAt: number;
  entityId: string;
  version: number;
  recordType: MkRecordType;
  status: string;
  title: string;
  data: Record<string, unknown>;
};

export type MkAgentProposal = {
  eventId: string;
  proposalId: string;
  targetId: string;
  targetKind: number;
  agent: string;
  proposalType: string;
  summary: string;
  provenance: string[];
  clips: Array<{ start: string; end: string; title: string; caption: string }>;
  createdAt: number;
};

export type MkIdeasSnapshot = {
  records: MkRecord[];
  proposals: MkAgentProposal[];
};

function parseObject(event: RelayEvent): Record<string, unknown> | null {
  try {
    const value: unknown = JSON.parse(event.content);
    return value !== null && typeof value === "object" && !Array.isArray(value)
      ? (value as Record<string, unknown>)
      : null;
  } catch {
    return null;
  }
}

function text(value: unknown): string {
  return typeof value === "string" ? value : "";
}

export function parseMkIdeasEvents(events: RelayEvent[]): MkIdeasSnapshot {
  const records: MkRecord[] = [];
  const proposals: MkAgentProposal[] = [];
  for (const event of events) {
    const data = parseObject(event);
    if (!data) continue;
    if (event.kind >= 30803 && event.kind <= 30809) {
      const recordType = text(data.record_type) as MkRecordType;
      if (
        !["person", "interview", "content", "approval"].includes(recordType)
      ) {
        continue;
      }
      const entityId = text(data.entity_id);
      const version = Number(data.version);
      if (!entityId || !Number.isInteger(version)) continue;
      records.push({
        eventId: event.id,
        kind: event.kind,
        author: event.pubkey,
        createdAt: event.created_at,
        entityId,
        version,
        recordType,
        status: text(data.status),
        title: text(data.name) || text(data.title) || "Untitled record",
        data,
      });
      continue;
    }
    if (event.kind === 48201) {
      const rawClips = Array.isArray(data.clips) ? data.clips : [];
      proposals.push({
        eventId: event.id,
        proposalId: text(data.proposal_id),
        targetId: text(data.target_id),
        targetKind: Number(data.target_kind),
        agent: text(data.agent),
        proposalType: text(data.proposal_type),
        summary: text(data.summary),
        provenance: Array.isArray(data.provenance)
          ? data.provenance.filter(
              (item): item is string => typeof item === "string",
            )
          : [],
        clips: rawClips.flatMap((clip) => {
          if (!clip || typeof clip !== "object" || Array.isArray(clip))
            return [];
          const value = clip as Record<string, unknown>;
          return [
            {
              start: text(value.start),
              end: text(value.end),
              title: text(value.title),
              caption: text(value.caption),
            },
          ];
        }),
        createdAt: event.created_at,
      });
    }
  }
  return {
    records: records.sort((a, b) => b.createdAt - a.createdAt),
    proposals: proposals.sort((a, b) => b.createdAt - a.createdAt),
  };
}

export function communityHost(relayUrl: string): string {
  const url = new URL(relayUrl);
  const defaultPort =
    (url.protocol === "wss:" && url.port === "443") ||
    (url.protocol === "ws:" && url.port === "80");
  return `${url.hostname.toLowerCase().replace(/\.$/, "")}${
    url.port && !defaultPort ? `:${url.port}` : ""
  }`;
}
