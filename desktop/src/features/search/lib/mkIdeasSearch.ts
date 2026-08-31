import type { SearchHit } from "@/shared/api/types";
import {
  KIND_MK_AGENT_PROPOSAL,
  KIND_MK_APPROVAL_ACTION,
  KIND_MK_EXTERNAL_COMMUNICATION,
  KIND_MK_GENERATED_SUMMARY,
  KIND_MK_MIGRATION_RECEIPT,
  KIND_MK_SYSTEM_ACTIVITY,
} from "@/shared/constants/kinds";
import {
  mkIdeasAreaForKind,
  mkIdeasLabelForKind,
  type MkIdeasArea,
} from "@/shared/lib/entityLink";
import { isMkStateKind, type MkStateKind } from "@/features/mkideas/model";

export type MkIdeasSearchPresentation = {
  area: MkIdeasArea;
  entityId?: string;
  label: string;
  schemaVersion: number;
  status?: string;
  title: string;
  version?: number;
};

const OPERATION_LABELS = new Map<number, string>([
  [KIND_MK_APPROVAL_ACTION, "Approval action"],
  [KIND_MK_AGENT_PROPOSAL, "Agent proposal"],
  [KIND_MK_MIGRATION_RECEIPT, "Migration receipt"],
  [KIND_MK_GENERATED_SUMMARY, "Generated summary"],
  [KIND_MK_SYSTEM_ACTIVITY, "System activity"],
  [KIND_MK_EXTERNAL_COMMUNICATION, "Communication history"],
]);

function parseObject(content: string): Record<string, unknown> | null {
  try {
    const value: unknown = JSON.parse(content);
    return value !== null && typeof value === "object" && !Array.isArray(value)
      ? (value as Record<string, unknown>)
      : null;
  } catch {
    return null;
  }
}

function text(value: unknown): string {
  return typeof value === "string" ? value.trim() : "";
}

function positiveInteger(value: unknown, fallback = 0): number {
  const parsed = Number(value);
  return Number.isInteger(parsed) && parsed > 0 ? parsed : fallback;
}

function stateTitle(kind: MkStateKind, data: Record<string, unknown>): string {
  return (
    text(data.name) ||
    text(data.title) ||
    text(data.subject) ||
    `Untitled ${mkIdeasLabelForKind(kind).toLowerCase()}`
  );
}

/** Build the typed row metadata used by universal search for MK events. */
export function parseMkIdeasSearchHit(
  hit: SearchHit,
): MkIdeasSearchPresentation | null {
  const data = parseObject(hit.content);
  if (!data) return null;

  if (isMkStateKind(hit.kind)) {
    const entityId = text(data.entity_id);
    const version = positiveInteger(data.version);
    if (!entityId || !version) return null;
    return {
      area: mkIdeasAreaForKind(hit.kind),
      entityId,
      label: mkIdeasLabelForKind(hit.kind),
      schemaVersion: positiveInteger(data.schema_version, 1),
      status: text(data.status) || undefined,
      title: stateTitle(hit.kind, data),
      version,
    };
  }

  const label = OPERATION_LABELS.get(hit.kind);
  if (!label) return null;
  const targetKind = positiveInteger(data.target_kind);
  const area = isMkStateKind(targetKind)
    ? mkIdeasAreaForKind(targetKind)
    : "today";
  const title =
    text(data.title) ||
    text(data.summary) ||
    text(data.agent) ||
    text(data.action) ||
    label;
  return {
    area,
    entityId:
      text(data.target_id) ||
      text(data.approval_id) ||
      text(data.receipt_id) ||
      undefined,
    label,
    schemaVersion: positiveInteger(data.schema_version, 1),
    status: text(data.status) || text(data.decision) || undefined,
    title,
  };
}

function isNewerHead(
  candidate: SearchHit,
  candidateVersion: number,
  current: SearchHit,
  currentVersion: number,
) {
  return (
    candidateVersion > currentVersion ||
    (candidateVersion === currentVersion &&
      (candidate.createdAt > current.createdAt ||
        (candidate.createdAt === current.createdAt &&
          candidate.eventId > current.eventId)))
  );
}

/**
 * Dedupe ordinary hits by event id and project MK state revisions to the
 * highest version in the returned search window. The winning head keeps the
 * first coordinate's result position, so relevance ordering stays stable.
 */
export function projectCurrentMkIdeasSearchHits(
  hits: SearchHit[],
): SearchHit[] {
  const projected: SearchHit[] = [];
  const eventIds = new Set<string>();
  const stateCoordinates = new Map<
    string,
    { index: number; hit: SearchHit; version: number }
  >();

  for (const hit of hits) {
    if (eventIds.has(hit.eventId)) continue;
    eventIds.add(hit.eventId);
    const presentation = parseMkIdeasSearchHit(hit);
    if (!presentation?.version || !presentation.entityId) {
      projected.push(hit);
      continue;
    }

    const coordinate = `${hit.kind}:${presentation.entityId}`;
    const current = stateCoordinates.get(coordinate);
    if (!current) {
      const index = projected.push(hit) - 1;
      stateCoordinates.set(coordinate, {
        index,
        hit,
        version: presentation.version,
      });
      continue;
    }
    if (isNewerHead(hit, presentation.version, current.hit, current.version)) {
      projected[current.index] = hit;
      stateCoordinates.set(coordinate, {
        ...current,
        hit,
        version: presentation.version,
      });
    }
  }
  return projected;
}

/** Resolve an MK search result to one of the permanent product areas. */
export function mkIdeasSearchArea(hit: SearchHit): MkIdeasArea | null {
  return parseMkIdeasSearchHit(hit)?.area ?? null;
}
