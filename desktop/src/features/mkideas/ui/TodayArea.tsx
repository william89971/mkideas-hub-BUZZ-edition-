import {
  AlertTriangle,
  ArrowUpRight,
  CalendarClock,
  CircleDot,
} from "lucide-react";

import type {
  MkActivity,
  MkAgentProposal,
  MkApprovalAction,
  MkGeneratedSummary,
  MkRecord,
} from "../model";
import { buildTodayInbox } from "../todayInbox";
import { buildMkIdeasLink } from "@/shared/lib/entityLink";
import { AgentWorkbench } from "./AgentWorkbench";
import {
  EmptyState,
  PageHeader,
  Status,
  TeamReferenceButton,
} from "./MkIdeasPrimitives";
import { ProposalCard } from "./ProposalCard";

function textList(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((item): item is string => typeof item === "string")
    : [];
}

export function TodayArea({
  records,
  proposals,
  approvalActions,
  activities,
  summaries,
  relayUrl,
  community,
  queryKey,
}: {
  records: MkRecord[];
  proposals: MkAgentProposal[];
  approvalActions: MkApprovalAction[];
  activities: MkActivity[];
  summaries: MkGeneratedSummary[];
  relayUrl: string;
  community: string;
  queryKey: readonly unknown[];
}) {
  const items = buildTodayInbox({
    records,
    proposals,
    approvalActions,
    activities,
  });
  const reviewActions = new Map(
    approvalActions.map((action) => [action.proposalId, action.decision]),
  );
  const briefing = summaries.find(
    (summary) => summary.summaryType === "operations_briefing",
  );
  const urgent = items.filter((item) => item.priority >= 85).length;
  const reviews = items.filter((item) => item.category === "approval").length;
  const upcoming = items.filter((item) => item.category === "upcoming").length;
  return (
    <div className="space-y-8">
      <PageHeader area="today" />
      <section className="grid gap-4 md:grid-cols-3">
        {[
          [String(reviews), "Human reviews", "Only a partner can decide"],
          [String(urgent), "Urgent signals", "Blocked, overdue, or failed"],
          [String(upcoming), "Coming up", "Meetings and interviews"],
        ].map(([value, label, note]) => (
          <div className="border border-border bg-card p-5" key={label}>
            <p className="font-serif text-3xl font-semibold text-foreground">
              {value}
            </p>
            <p className="mt-2 text-sm font-medium text-foreground">{label}</p>
            <p className="mt-1 text-xs text-muted-foreground">{note}</p>
          </div>
        ))}
      </section>

      <section className="grid gap-8 xl:grid-cols-[1.35fr_0.65fr]">
        <div>
          <div className="mb-3 flex items-end justify-between gap-4">
            <div>
              <p className="text-2xs font-semibold uppercase tracking-widest text-[#b70f22]">
                Ranked and deduplicated
              </p>
              <h2 className="mt-1 font-serif text-2xl font-semibold">
                Operational inbox
              </h2>
            </div>
            <span className="text-xs text-muted-foreground">
              {items.length} actionable records
            </span>
          </div>
          <div className="space-y-3">
            {items.length ? (
              items.map((item, index) =>
                item.category === "approval" &&
                item.proposal &&
                item.approval ? (
                  <div key={item.key}>
                    <div className="mb-1 flex items-center gap-2 text-2xs font-semibold uppercase tracking-widest text-[#b70f22]">
                      <span>{String(index + 1).padStart(2, "0")}</span>
                      <span>Human decision</span>
                    </div>
                    <ProposalCard
                      approval={item.approval}
                      community={community}
                      compact
                      proposal={item.proposal}
                      queryKey={queryKey}
                      relayUrl={relayUrl}
                      reviewedDecision={reviewActions.get(
                        item.proposal.proposalId,
                      )}
                    />
                  </div>
                ) : (
                  <article
                    className="grid grid-cols-[2.5rem_1fr_auto] gap-3 border border-border bg-card p-4"
                    key={item.key}
                  >
                    <span className="font-mono text-xs text-[#b70f22]">
                      {String(index + 1).padStart(2, "0")}
                    </span>
                    <div className="min-w-0">
                      <div className="flex flex-wrap items-center gap-2">
                        <h3 className="font-serif text-lg font-semibold">
                          {item.title}
                        </h3>
                        <Status>{item.category}</Status>
                      </div>
                      <p className="mt-1 text-xs leading-5 text-muted-foreground">
                        {item.detail}
                      </p>
                      <p className="mt-2 text-2xs font-medium uppercase tracking-wide text-muted-foreground">
                        {item.signals.join(" · ")}
                      </p>
                      {item.record ? (
                        <div className="mt-2">
                          <TeamReferenceButton
                            label={item.record.title}
                            reference={buildMkIdeasLink({
                              community,
                              kind: item.record.kind,
                              id: item.record.entityId,
                            })}
                          />
                        </div>
                      ) : null}
                    </div>
                    {item.category === "blocked" || item.priority >= 90 ? (
                      <AlertTriangle className="h-4 w-4 text-[#b70f22]" />
                    ) : item.category === "upcoming" ? (
                      <CalendarClock className="h-4 w-4 text-[#b70f22]" />
                    ) : (
                      <CircleDot className="h-4 w-4 text-[#b70f22]" />
                    )}
                  </article>
                ),
              )
            ) : (
              <EmptyState>
                No work needs a partner’s attention right now.
              </EmptyState>
            )}
          </div>
        </div>

        <aside>
          <div className="mb-3 flex items-center justify-between">
            <h2 className="font-serif text-2xl font-semibold">Daily brief</h2>
            <Status>informational only</Status>
          </div>
          {briefing ? (
            <div className="border border-border bg-card p-5">
              {[
                ["Approvals", textList(briefing.output.approvals)],
                ["Deadlines", textList(briefing.output.deadlines)],
                ["Blocked", textList(briefing.output.blocked_work)],
                ["Next", textList(briefing.output.recommendations)],
              ].map(([label, values]) => (
                <div
                  className="border-t border-border py-3 first:border-t-0 first:pt-0"
                  key={String(label)}
                >
                  <p className="text-2xs font-semibold uppercase tracking-widest text-[#b70f22]">
                    {String(label)}
                  </p>
                  {(values as string[]).map((value) => (
                    <p className="mt-1 text-xs leading-5" key={value}>
                      {value}
                    </p>
                  ))}
                </div>
              ))}
              <p className="mt-3 flex items-center gap-1 text-2xs text-muted-foreground">
                <ArrowUpRight className="h-3.5 w-3.5" />
                {briefing.provenance.length} sourced snapshot
              </p>
            </div>
          ) : (
            <EmptyState>
              The operations briefing will appear after its first run.
            </EmptyState>
          )}
        </aside>
      </section>

      <AgentWorkbench
        activities={activities}
        personaIds={["operations-briefing-assistant"]}
        proposals={proposals}
      />
    </div>
  );
}
