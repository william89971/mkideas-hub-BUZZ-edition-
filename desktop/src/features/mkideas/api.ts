import { relayClient } from "@/shared/api/relayClient";
import { signRelayEvent } from "@/shared/api/tauri";
import {
  KIND_MK_AGENT_PROPOSAL,
  KIND_MK_APPROVAL,
  KIND_MK_APPROVAL_ACTION,
  KIND_MK_CONTENT,
  KIND_MK_INTERVIEW,
  KIND_MK_PERSON,
} from "@/shared/constants/kinds";
import type { RelayEvent } from "@/shared/api/types";
import {
  communityHost,
  parseMkIdeasEvents,
  type MkIdeasSnapshot,
  type MkRecord,
} from "./model";

const V0_KINDS = [
  KIND_MK_PERSON,
  KIND_MK_INTERVIEW,
  KIND_MK_CONTENT,
  KIND_MK_APPROVAL,
  KIND_MK_APPROVAL_ACTION,
  KIND_MK_AGENT_PROPOSAL,
];

export async function fetchMkIdeasSnapshot(
  relayUrl: string,
): Promise<MkIdeasSnapshot> {
  const host = communityHost(relayUrl);
  const events = await relayClient.fetchEvents({
    kinds: V0_KINDS,
    "#h": [host],
    limit: 500,
  });
  return parseMkIdeasEvents(events);
}

export async function subscribeMkIdeas(
  relayUrl: string,
  onEvent: (event: RelayEvent) => void,
): Promise<() => Promise<void>> {
  return relayClient.subscribeLive(
    {
      kinds: V0_KINDS,
      "#h": [communityHost(relayUrl)],
      since: Math.floor(Date.now() / 1000),
      limit: 0,
    },
    onEvent,
  );
}

type StateInput = {
  kind: number;
  recordType: "person" | "interview" | "content" | "approval";
  entityId?: string;
  previous?: MkRecord;
  status: string;
  fields: Record<string, unknown>;
};

export async function publishMkState(relayUrl: string, input: StateInput) {
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
  if (
    input.kind === KIND_MK_INTERVIEW &&
    typeof input.fields.guest_id === "string"
  ) {
    tags.push(["guest", input.fields.guest_id]);
  }
  if (
    input.kind === KIND_MK_CONTENT &&
    typeof input.fields.interview_id === "string"
  ) {
    tags.push(["interview", input.fields.interview_id]);
  }
  if (
    input.kind === KIND_MK_APPROVAL &&
    typeof input.fields.target_id === "string" &&
    typeof input.fields.target_kind === "number" &&
    typeof input.fields.proposal_id === "string" &&
    typeof input.fields.proposal_event_id === "string"
  ) {
    tags.push([
      "target",
      String(input.fields.target_kind),
      input.fields.target_id,
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
      schema_version: 1,
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

export async function publishApprovalDecision(
  relayUrl: string,
  proposalEventId: string,
  proposalId: string,
  targetId: string,
  targetKind: number,
  decision: "approved" | "rejected",
) {
  const approvalId = proposalId;
  await publishMkState(relayUrl, {
    kind: KIND_MK_APPROVAL,
    recordType: "approval",
    entityId: approvalId,
    status: decision,
    fields: {
      target_id: targetId,
      target_kind: targetKind,
      proposal_id: proposalId,
      proposal_event_id: proposalEventId,
    },
  });
  const action = await signRelayEvent({
    kind: KIND_MK_APPROVAL_ACTION,
    tags: [
      ["h", communityHost(relayUrl)],
      ["e", proposalEventId],
    ],
    content: JSON.stringify({
      schema_version: 1,
      approval_id: approvalId,
      target_id: targetId,
      proposal_id: proposalId,
      decision,
    }),
  });
  await relayClient.publishEvent(
    action,
    "Timed out while recording the review.",
    "MK Ideas could not record the review.",
  );
}
