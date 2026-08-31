import type {
  MkActivity,
  MkAgentProposal,
  MkApprovalAction,
  MkRecord,
} from "./model";

export type MkTodayCategory =
  | "approval"
  | "deadline"
  | "blocked"
  | "upcoming"
  | "assignment"
  | "mention"
  | "agent"
  | "change";

export type MkTodayItem = {
  key: string;
  category: MkTodayCategory;
  priority: number;
  title: string;
  detail: string;
  signals: string[];
  record?: MkRecord;
  proposal?: MkAgentProposal;
  approval?: MkRecord<"approval">;
};

function asStringList(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((item): item is string => typeof item === "string")
    : typeof value === "string" && value
      ? [value]
      : [];
}

function timestamp(value: unknown): number | null {
  if (typeof value !== "string" || !value) return null;
  const parsed = Date.parse(value);
  return Number.isFinite(parsed) ? parsed : null;
}

function entityKey(kind: number, id: string): string {
  return `${kind}:${id}`;
}

function titleForTarget(records: MkRecord[], kind: number, id: string) {
  return records.find(
    (record) => record.kind === kind && record.entityId === id,
  );
}

export function buildTodayInbox({
  records,
  proposals,
  approvalActions,
  activities,
  now = Date.now(),
}: {
  records: MkRecord[];
  proposals: MkAgentProposal[];
  approvalActions: MkApprovalAction[];
  activities: MkActivity[];
  now?: number;
}): MkTodayItem[] {
  const candidates: MkTodayItem[] = [];
  const proposalsById = new Map(
    proposals.map((proposal) => [proposal.proposalId, proposal]),
  );
  const decidedApprovals = new Set(
    approvalActions.map((action) => action.approvalId),
  );
  for (const rawApproval of records.filter(
    (record) => record.recordType === "approval" && record.status === "pending",
  )) {
    const approval = rawApproval as MkRecord<"approval">;
    if (decidedApprovals.has(approval.entityId)) continue;
    const proposalId = String(approval.data.proposal_id ?? "");
    const proposal = proposalsById.get(proposalId);
    const targetKind = Number(
      approval.data.target_kind ?? proposal?.targetKind,
    );
    const targetId = String(
      approval.data.target_id ?? proposal?.targetId ?? "",
    );
    const target = titleForTarget(records, targetKind, targetId);
    candidates.push({
      key: entityKey(targetKind, targetId || approval.entityId),
      category: "approval",
      priority: 100,
      title: target?.title ?? proposal?.summary ?? approval.title,
      detail: "A signed human decision is required.",
      signals: ["approval waiting"],
      record: target,
      proposal,
      approval,
    });
  }

  for (const record of records) {
    if (record.recordType === "approval") continue;
    const key = entityKey(record.kind, record.entityId);
    if (record.status === "blocked") {
      candidates.push({
        key,
        category: "blocked",
        priority: 90,
        title: record.title,
        detail: String(record.data.blocked_reason ?? "Work is blocked."),
        signals: ["blocked"],
        record,
      });
    }
    const due = timestamp(record.data.due_at);
    if (
      due !== null &&
      !["done", "completed", "cancelled", "archived"].includes(record.status)
    ) {
      const hours = (due - now) / 3_600_000;
      if (hours <= 72) {
        candidates.push({
          key,
          category: "deadline",
          priority: hours < 0 ? 95 : hours <= 24 ? 84 : 72,
          title: record.title,
          detail:
            hours < 0
              ? "Deadline has passed."
              : `Due ${new Date(due).toLocaleString()}.`,
          signals: [hours < 0 ? "overdue" : "deadline"],
          record,
        });
      }
    }
    const scheduled = timestamp(
      record.data.scheduled_at ?? record.data.starts_at,
    );
    if (
      scheduled !== null &&
      scheduled >= now &&
      scheduled - now <= 7 * 86_400_000 &&
      (record.recordType === "meeting" || record.recordType === "interview")
    ) {
      candidates.push({
        key,
        category: "upcoming",
        priority: 65,
        title: record.title,
        detail: `Scheduled ${new Date(scheduled).toLocaleString()}.`,
        signals: ["upcoming"],
        record,
      });
    }
    const assignees = asStringList(
      record.data.assignees ?? record.data.assignee,
    );
    if (
      assignees.length &&
      !["done", "completed", "cancelled", "archived"].includes(record.status)
    ) {
      candidates.push({
        key,
        category: "assignment",
        priority: 55,
        title: record.title,
        detail: `Assigned to ${assignees.join(", ")}.`,
        signals: ["assigned"],
        record,
      });
    }
  }

  for (const proposal of proposals) {
    if (
      records.some(
        (record) =>
          record.recordType === "approval" &&
          record.data.proposal_id === proposal.proposalId,
      )
    ) {
      continue;
    }
    const target = titleForTarget(
      records,
      proposal.targetKind,
      proposal.targetId,
    );
    candidates.push({
      key: entityKey(proposal.targetKind, proposal.targetId),
      category: "agent",
      priority: proposal.status === "failed" ? 92 : 70,
      title: target?.title ?? proposal.summary,
      detail:
        proposal.status === "failed"
          ? `${proposal.agent} needs attention.`
          : `${proposal.agent} produced a draft for review.`,
      signals: [proposal.status === "failed" ? "agent failed" : "agent result"],
      record: target,
      proposal,
    });
  }

  for (const activity of activities) {
    if (!activity.summary && !activity.targetId) continue;
    const target = titleForTarget(
      records,
      activity.targetKind,
      activity.targetId,
    );
    const isMention = activity.activityType === "mention";
    const isFailure = activity.status === "failed";
    candidates.push({
      key:
        activity.targetId && activity.targetKind
          ? entityKey(activity.targetKind, activity.targetId)
          : `activity:${activity.eventId}`,
      category: isMention ? "mention" : isFailure ? "agent" : "change",
      priority: isFailure ? 92 : isMention ? 78 : 45,
      title: target?.title ?? activity.summary ?? "Important change",
      detail:
        activity.summary || `${activity.activityType.replaceAll("_", " ")}.`,
      signals: [
        isMention ? "mentioned" : isFailure ? "agent failed" : "changed",
      ],
      record: target,
    });
  }

  const deduped = new Map<string, MkTodayItem>();
  for (const candidate of candidates.sort((a, b) => b.priority - a.priority)) {
    const existing = deduped.get(candidate.key);
    if (!existing) {
      deduped.set(candidate.key, candidate);
      continue;
    }
    existing.signals = [
      ...new Set([...existing.signals, ...candidate.signals]),
    ];
  }
  return [...deduped.values()].sort(
    (a, b) => b.priority - a.priority || a.title.localeCompare(b.title),
  );
}
