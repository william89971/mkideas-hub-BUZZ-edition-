import { relayClient } from "@/shared/api/relayClient";
import { signRelayEvent } from "@/shared/api/tauri";
import type { RelayEvent } from "@/shared/api/types";
import {
  KIND_MK_AGENT_PROPOSAL,
  KIND_MK_APPROVAL,
  KIND_MK_APPROVAL_ACTION,
  KIND_MK_EXTERNAL_COMMUNICATION,
  KIND_MK_GENERATED_SUMMARY,
  KIND_MK_MIGRATION_RECEIPT,
  KIND_MK_SYSTEM_ACTIVITY,
} from "@/shared/constants/kinds";
import {
  communityHost,
  MK_RECORD_KIND_BY_TYPE,
  parseMkIdeasEvents,
  type MkAgentProposal,
  type MkIdeasSnapshot,
  type MkRecord,
  type MkRecordStatus,
  type MkRecordType,
  type MkStateKind,
} from "./model";
import {
  collectMkProjection,
  queryMkProjection,
  type MkProjectionPage,
  type MkProjectionTransport,
} from "./projectionApi";

const MK_STATE_KINDS = Object.values(MK_RECORD_KIND_BY_TYPE);
const MK_OPERATION_KINDS = [
  KIND_MK_APPROVAL_ACTION,
  KIND_MK_AGENT_PROPOSAL,
  KIND_MK_MIGRATION_RECEIPT,
  KIND_MK_GENERATED_SUMMARY,
  KIND_MK_SYSTEM_ACTIVITY,
  KIND_MK_EXTERNAL_COMMUNICATION,
];
const MK_QUERY_KINDS = [...MK_STATE_KINDS, ...MK_OPERATION_KINDS];

export async function fetchMkIdeasSnapshot(
  relayUrl: string,
  projectionTransport: MkProjectionTransport = queryMkProjection,
): Promise<MkIdeasSnapshot> {
  const host = communityHost(relayUrl);
  const [heads, operations] = await Promise.all([
    collectMkProjection(
      {
        projection: "heads",
        kinds: [...MK_STATE_KINDS],
        community: host,
        limit: 200,
      },
      projectionTransport,
    ),
    relayClient.fetchEvents({
      kinds: MK_OPERATION_KINDS,
      "#h": [host],
      limit: 200,
    }),
  ]);
  return parseMkIdeasEvents([...heads, ...operations]);
}

export async function fetchMkEntityHistoryPage(
  relayUrl: string,
  input: {
    kind: MkStateKind;
    entityId: string;
    cursor?: string;
    limit?: number;
  },
  projectionTransport: MkProjectionTransport = queryMkProjection,
): Promise<{ records: MkRecord[]; nextCursor: string | null }> {
  const page: MkProjectionPage = await projectionTransport({
    projection: "history",
    kinds: [input.kind],
    community: communityHost(relayUrl),
    entityId: input.entityId,
    cursor: input.cursor,
    limit: input.limit ?? 20,
  });
  return {
    records: parseMkIdeasEvents(page.events).records,
    nextCursor: page.nextCursor,
  };
}

export async function subscribeMkIdeas(
  relayUrl: string,
  onEvent: (event: RelayEvent) => void,
): Promise<() => Promise<void>> {
  return relayClient.subscribeLive(
    {
      kinds: MK_QUERY_KINDS,
      "#h": [communityHost(relayUrl)],
      since: Math.floor(Date.now() / 1000),
      limit: 0,
    },
    onEvent,
  );
}

export type MkStateInput = {
  kind: MkStateKind;
  recordType: MkRecordType;
  entityId?: string;
  previous?: MkRecord;
  status: MkRecordStatus;
  fields: Record<string, unknown>;
};

function appendTypedTags(
  tags: string[][],
  fields: Record<string, unknown>,
): void {
  const scalarLinks = [
    "goal_id",
    "project_id",
    "parent_id",
    "guest_id",
    "interview_id",
    "content_id",
  ];
  for (const field of scalarLinks) {
    const value = fields[field];
    if (typeof value === "string" && value) tags.push([field, value]);
  }
  const dueAt = fields.due_at;
  if (typeof dueAt === "string" && dueAt) tags.push(["due", dueAt]);
  const assignees = Array.isArray(fields.assignees)
    ? fields.assignees
    : typeof fields.assignee === "string"
      ? [fields.assignee]
      : [];
  for (const assignee of assignees) {
    if (typeof assignee === "string" && assignee) {
      tags.push(["assignee", assignee]);
    }
  }
}

export async function publishMkState(relayUrl: string, input: MkStateInput) {
  const entityId =
    input.entityId ?? input.previous?.entityId ?? crypto.randomUUID();
  const version = (input.previous?.version ?? 0) + 1;
  const tags = [
    ["d", entityId],
    ["h", communityHost(relayUrl)],
    ["version", String(version)],
    ["status", input.status],
  ];
  if (input.previous) tags.push(["prev", input.previous.eventId]);
  appendTypedTags(tags, input.fields);
  if (
    input.kind === KIND_MK_APPROVAL &&
    typeof input.fields.target_id === "string" &&
    typeof input.fields.target_kind === "number" &&
    typeof input.fields.target_event_id === "string" &&
    typeof input.fields.proposal_id === "string" &&
    typeof input.fields.proposal_event_id === "string"
  ) {
    tags.push([
      "target",
      String(input.fields.target_kind),
      input.fields.target_id,
      input.fields.target_event_id,
    ]);
    tags.push([
      "proposal",
      input.fields.proposal_id,
      input.fields.proposal_event_id,
    ]);
  }
  const event = await signRelayEvent({
    kind: input.kind,
    tags,
    content: JSON.stringify({
      ...input.fields,
      source:
        typeof input.fields.source === "string" && input.fields.source
          ? input.fields.source
          : "buzz-desktop-human",
      provenance:
        input.fields.provenance &&
        typeof input.fields.provenance === "object" &&
        !Array.isArray(input.fields.provenance)
          ? input.fields.provenance
          : {
              type: "human_entry",
              ref: input.previous?.eventId ?? entityId,
            },
      schema_version: 2,
      record_type: input.recordType,
      entity_id: entityId,
      version,
      status: input.status,
    }),
  });
  await relayClient.publishEvent(
    event,
    "Timed out while saving MK Ideas work.",
    "MK Ideas could not save this change.",
  );
  return event;
}

export type MkAtomicApprovalDecisionInput = {
  approval: MkRecord<"approval">;
  proposal: MkAgentProposal;
  decision: "approved" | "rejected";
  reason: string;
  resultEvent?: RelayEvent;
};

function requiredText(data: Record<string, unknown>, field: string): string {
  const value = data[field];
  if (typeof value !== "string" || !value) {
    throw new Error(`Approval request is missing ${field}.`);
  }
  return value;
}

function requiredPositiveInteger(
  data: Record<string, unknown>,
  field: string,
): number {
  const value = Number(data[field]);
  if (!Number.isInteger(value) || value < 1) {
    throw new Error(`Approval request is missing ${field}.`);
  }
  return value;
}

export function buildAtomicApprovalAction(
  relayUrl: string,
  input: MkAtomicApprovalDecisionInput,
): { kind: number; tags: string[][]; content: string } {
  const { approval, proposal } = input;
  if (approval.status !== "pending") {
    throw new Error("Only a pending approval request can be decided.");
  }
  const targetId = requiredText(approval.data, "target_id");
  const targetKind = requiredPositiveInteger(approval.data, "target_kind");
  const targetEventId = requiredText(approval.data, "target_event_id");
  const targetVersion = requiredPositiveInteger(
    approval.data,
    "target_version",
  );
  const proposalId = requiredText(approval.data, "proposal_id");
  const proposalEventId = requiredText(approval.data, "proposal_event_id");
  if (
    proposalId !== proposal.proposalId ||
    proposalEventId !== proposal.eventId ||
    targetId !== proposal.targetId ||
    targetKind !== proposal.targetKind ||
    targetEventId !== proposal.targetEventId ||
    targetVersion !== proposal.targetVersion
  ) {
    throw new Error(
      "Approval request no longer matches the exact proposal target.",
    );
  }
  const reason = input.reason.trim();
  if (!reason) throw new Error("A review reason is required.");
  const body: Record<string, unknown> = {
    schema_version: 2,
    action_id: crypto.randomUUID(),
    approval_id: approval.entityId,
    approval_event_id: approval.eventId,
    target_id: targetId,
    target_kind: targetKind,
    target_event_id: targetEventId,
    target_version: targetVersion,
    proposal_id: proposalId,
    proposal_event_id: proposalEventId,
    decision: input.decision,
    reason,
  };
  if (input.resultEvent) body.result_event = input.resultEvent;
  return {
    kind: KIND_MK_APPROVAL_ACTION,
    tags: [
      ["h", communityHost(relayUrl)],
      ["approval", approval.entityId, approval.eventId],
      ["proposal", proposalId, proposalEventId],
      ["target", String(targetKind), targetId, targetEventId],
    ],
    content: JSON.stringify(body),
  };
}

export async function publishApprovalDecision(
  relayUrl: string,
  input: MkAtomicApprovalDecisionInput,
) {
  const action = await signRelayEvent(
    buildAtomicApprovalAction(relayUrl, input),
  );
  await relayClient.publishEvent(
    action,
    "Timed out while recording the review.",
    "MK Ideas could not record the review.",
  );
  return action;
}
