import type { RelayEvent } from "@/shared/api/types";
import {
  KIND_MK_APPROVAL_ACTION,
  KIND_MK_APPROVAL,
  KIND_MK_AGENT_PROPOSAL,
  KIND_MK_CONTENT,
  KIND_MK_DECISION,
  KIND_MK_GOAL,
  KIND_MK_INTERVIEW,
  KIND_MK_KNOWLEDGE,
  KIND_MK_MEETING,
  KIND_MK_OPERATIONAL_PROJECT,
  KIND_MK_PERSON,
  KIND_MK_TASK,
  KIND_MK_GENERATED_SUMMARY,
  KIND_MK_SYSTEM_ACTIVITY,
} from "@/shared/constants/kinds";

export const MK_RECORD_TYPES = [
  "goal",
  "project",
  "task",
  "person",
  "interview",
  "content",
  "meeting",
  "decision",
  "knowledge",
  "approval",
] as const;

export type MkRecordType = (typeof MK_RECORD_TYPES)[number];

export const MK_RECORD_KIND_BY_TYPE = {
  goal: KIND_MK_GOAL,
  project: KIND_MK_OPERATIONAL_PROJECT,
  task: KIND_MK_TASK,
  person: KIND_MK_PERSON,
  interview: KIND_MK_INTERVIEW,
  content: KIND_MK_CONTENT,
  meeting: KIND_MK_MEETING,
  decision: KIND_MK_DECISION,
  knowledge: KIND_MK_KNOWLEDGE,
  approval: KIND_MK_APPROVAL,
} as const satisfies Record<MkRecordType, number>;

export type MkStateKind = (typeof MK_RECORD_KIND_BY_TYPE)[MkRecordType];

export const MK_STATUSES = {
  goal: ["draft", "active", "on-hold", "completed", "archived"],
  project: ["planned", "active", "blocked", "completed", "archived"],
  task: [
    "backlog",
    "to-do",
    "in-progress",
    "blocked",
    "review",
    "done",
    "cancelled",
  ],
  person: [
    "prospect",
    "researching",
    "ready-to-contact",
    "contacted",
    "responded",
    "scheduled",
    "interviewed",
    "nurture",
    "closed",
    "archived",
  ],
  interview: [
    "idea",
    "planning",
    "scheduled",
    "recorded",
    "transcribing",
    "reviewing",
    "complete",
    "cancelled",
  ],
  content: [
    "idea",
    "draft",
    "in-review",
    "approved",
    "scheduled",
    "published",
    "archived",
  ],
  meeting: ["planned", "completed", "cancelled"],
  decision: ["proposed", "decided", "superseded", "archived"],
  knowledge: ["draft", "verified", "archived"],
  approval: ["pending", "approved", "rejected", "stale", "cancelled"],
} as const satisfies Record<MkRecordType, readonly string[]>;

export type MkStatusFor<T extends MkRecordType> =
  (typeof MK_STATUSES)[T][number];
export type MkRecordStatus = MkStatusFor<MkRecordType>;

export type MkRecord<T extends MkRecordType = MkRecordType> = {
  eventId: string;
  kind: MkStateKind;
  author: string;
  createdAt: number;
  entityId: string;
  version: number;
  schemaVersion: number;
  recordType: T;
  status: MkStatusFor<T>;
  rawStatus: string;
  title: string;
  data: Record<string, unknown>;
};

export type MkAgentProposal = {
  eventId: string;
  proposalId: string;
  targetId: string;
  targetKind: number;
  targetEventId: string;
  targetVersion?: number;
  schemaVersion: number;
  agent: string;
  personaId: string;
  proposalType: string;
  proposalVersion: number;
  runId: string;
  templateVersion: string;
  provider: string;
  model: string;
  status: string;
  reviewState: string;
  inputEventIds: string[];
  inputHash: string;
  summary: string;
  provenance: string[];
  clips: Array<{ start: string; end: string; title: string; caption: string }>;
  output: Record<string, unknown>;
  startedAt: string;
  completedAt: string;
  createdAt: number;
};

export type MkApprovalAction = {
  eventId: string;
  actionId: string;
  approvalId: string;
  approvalEventId: string;
  targetId: string;
  targetKind: number;
  targetEventId: string;
  targetVersion: number;
  proposalId: string;
  proposalEventId: string;
  decision: "approved" | "rejected";
  reason: string;
  createdAt: number;
  author: string;
};

export type MkActivity = {
  eventId: string;
  activityType: string;
  runId: string;
  personaId: string;
  status: string;
  targetId: string;
  targetKind: number;
  summary: string;
  createdAt: number;
};

export type MkGeneratedSummary = {
  eventId: string;
  summaryId: string;
  summaryType: string;
  personaId: string;
  status: string;
  output: Record<string, unknown>;
  provenance: string[];
  createdAt: number;
};

export type MkIdeasSnapshot = {
  records: MkRecord[];
  proposals: MkAgentProposal[];
  approvalActions: MkApprovalAction[];
  activities: MkActivity[];
  summaries: MkGeneratedSummary[];
};

const TYPE_BY_KIND = new Map<number, MkRecordType>(
  Object.entries(MK_RECORD_KIND_BY_TYPE).map(([type, kind]) => [
    kind,
    type as MkRecordType,
  ]),
);

const LEGACY_STATUS_ALIASES: Partial<
  Record<MkRecordType, Record<string, string>>
> = {
  person: { potential: "prospect", research_ready: "ready-to-contact" },
  interview: { content_processing: "transcribing" },
  content: { internal_review: "in-review" },
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

function stringList(value: unknown): string[] {
  return Array.isArray(value)
    ? value.flatMap((item) => {
        if (typeof item === "string") return [item];
        if (!item || typeof item !== "object" || Array.isArray(item)) return [];
        const source = item as Record<string, unknown>;
        return [
          text(source.title) ||
            text(source.locator) ||
            text(source.source_id) ||
            text(source.source_type),
        ].filter(Boolean);
      })
    : [];
}

function nestedObject(
  data: Record<string, unknown>,
  field: string,
): Record<string, unknown> {
  const value = data[field];
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};
}

function integer(value: unknown, fallback = 0): number {
  const parsed = Number(value);
  return Number.isInteger(parsed) ? parsed : fallback;
}

function firstTag(event: RelayEvent, name: string): string {
  return event.tags.find((tag) => tag[0] === name)?.[1] ?? "";
}

export function mkRecordTypeForKind(kind: number): MkRecordType | undefined {
  return TYPE_BY_KIND.get(kind);
}

export function isMkStateKind(kind: number): kind is MkStateKind {
  return TYPE_BY_KIND.has(kind);
}

export function isMkStatusFor<T extends MkRecordType>(
  recordType: T,
  status: string,
): status is MkStatusFor<T> {
  return (MK_STATUSES[recordType] as readonly string[]).includes(status);
}

function normalizedStatus<T extends MkRecordType>(
  recordType: T,
  rawStatus: string,
): MkStatusFor<T> | null {
  const status = LEGACY_STATUS_ALIASES[recordType]?.[rawStatus] ?? rawStatus;
  return isMkStatusFor(recordType, status) ? status : null;
}

function recordTitle(
  recordType: MkRecordType,
  data: Record<string, unknown>,
): string {
  const explicit = text(data.name) || text(data.title) || text(data.subject);
  if (explicit) return explicit;
  return recordType === "approval" ? "Human review" : `Untitled ${recordType}`;
}

export function parseMkIdeasEvents(events: RelayEvent[]): MkIdeasSnapshot {
  const records: MkRecord[] = [];
  const proposals: MkAgentProposal[] = [];
  const approvalActions: MkApprovalAction[] = [];
  const activities: MkActivity[] = [];
  const summaries: MkGeneratedSummary[] = [];
  for (const event of events) {
    const data = parseObject(event);
    if (!data) continue;
    const kindRecordType = mkRecordTypeForKind(event.kind);
    if (kindRecordType) {
      const declaredType = text(data.record_type);
      const recordType = MK_RECORD_TYPES.includes(declaredType as MkRecordType)
        ? (declaredType as MkRecordType)
        : kindRecordType;
      if (recordType !== kindRecordType) continue;

      const entityId = text(data.entity_id) || firstTag(event, "d");
      const version = integer(
        data.version,
        integer(firstTag(event, "version")),
      );
      const schemaVersion = integer(data.schema_version, 1);
      const rawStatus = text(data.status) || firstTag(event, "status");
      const status = normalizedStatus(recordType, rawStatus);
      if (!entityId || version < 1 || schemaVersion < 1 || !status) continue;

      records.push({
        eventId: event.id,
        kind: event.kind as MkStateKind,
        author: event.pubkey,
        createdAt: event.created_at,
        entityId,
        version,
        schemaVersion,
        recordType,
        status,
        rawStatus,
        title: recordTitle(recordType, data),
        data,
      });
      continue;
    }
    if (event.kind === KIND_MK_AGENT_PROPOSAL) {
      const rawClips = Array.isArray(data.clips) ? data.clips : [];
      const target = nestedObject(data, "target");
      const output = nestedObject(data, "output");
      const outputClips = Array.isArray(output.clips) ? output.clips : [];
      proposals.push({
        eventId: event.id,
        proposalId: text(data.proposal_id),
        targetId: text(data.target_id) || text(target.id),
        targetKind: integer(data.target_kind, integer(target.kind)),
        targetEventId: text(data.target_event_id) || text(target.event_id),
        targetVersion:
          integer(data.target_version, integer(target.version)) > 0
            ? integer(data.target_version, integer(target.version))
            : undefined,
        schemaVersion: integer(data.schema_version, 1),
        agent: text(data.agent) || text(data.persona_id).replaceAll("-", " "),
        personaId: text(data.persona_id),
        proposalType: text(data.proposal_type),
        proposalVersion: integer(data.proposal_version, 1),
        runId: text(data.run_id),
        templateVersion: text(data.template_version),
        provider: text(data.provider),
        model: text(data.model),
        status: text(data.status) || firstTag(event, "status"),
        reviewState: text(data.review_state) || "pending",
        inputEventIds: stringList(data.input_event_ids),
        inputHash: text(data.input_hash),
        summary: text(data.summary),
        provenance: stringList(data.provenance),
        clips: [...rawClips, ...outputClips].flatMap((clip) => {
          if (!clip || typeof clip !== "object" || Array.isArray(clip)) {
            return [];
          }
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
        output,
        startedAt: text(data.started_at),
        completedAt: text(data.completed_at),
        createdAt: event.created_at,
      });
      continue;
    }
    if (event.kind === KIND_MK_APPROVAL_ACTION) {
      const decision = text(data.decision);
      if (decision !== "approved" && decision !== "rejected") continue;
      approvalActions.push({
        eventId: event.id,
        actionId: text(data.action_id),
        approvalId: text(data.approval_id),
        approvalEventId: text(data.approval_event_id),
        targetId: text(data.target_id),
        targetKind: integer(data.target_kind),
        targetEventId: text(data.target_event_id),
        targetVersion: integer(data.target_version),
        proposalId: text(data.proposal_id),
        proposalEventId: text(data.proposal_event_id),
        decision,
        reason: text(data.reason),
        createdAt: event.created_at,
        author: event.pubkey,
      });
      continue;
    }
    if (event.kind === KIND_MK_SYSTEM_ACTIVITY) {
      const target = nestedObject(data, "target");
      activities.push({
        eventId: event.id,
        activityType: text(data.activity_type),
        runId: text(data.run_id),
        personaId: text(data.persona_id),
        status: text(data.status),
        targetId: text(data.target_id) || text(target.id),
        targetKind: integer(data.target_kind, integer(target.kind)),
        summary: text(data.summary),
        createdAt: event.created_at,
      });
      continue;
    }
    if (event.kind === KIND_MK_GENERATED_SUMMARY) {
      summaries.push({
        eventId: event.id,
        summaryId: text(data.summary_id),
        summaryType: text(data.summary_type),
        personaId: text(data.persona_id),
        status: text(data.status),
        output: nestedObject(data, "output"),
        provenance: stringList(data.provenance),
        createdAt: event.created_at,
      });
    }
  }
  return {
    records: records.sort((a, b) => b.createdAt - a.createdAt),
    proposals: proposals.sort((a, b) => b.createdAt - a.createdAt),
    approvalActions: approvalActions.sort((a, b) => b.createdAt - a.createdAt),
    activities: activities.sort((a, b) => b.createdAt - a.createdAt),
    summaries: summaries.sort((a, b) => b.createdAt - a.createdAt),
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
