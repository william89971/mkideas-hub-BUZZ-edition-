import { Bot, ExternalLink, ShieldCheck } from "lucide-react";

import { requestOpenCreateAgent } from "@/features/agents/openCreateAgentEvent";
import type { MkActivity, MkAgentProposal } from "../model";
import { Button } from "@/shared/ui/button";
import { Status } from "./MkIdeasPrimitives";

export type MkAgentPersonaId =
  | "guest-researcher"
  | "outreach-drafter"
  | "interview-producer"
  | "content-clip-copilot"
  | "operations-briefing-assistant";

const PERSONA_COPY: Record<
  MkAgentPersonaId,
  { name: string; purpose: string; safeguard: string }
> = {
  "guest-researcher": {
    name: "Guest Researcher",
    purpose: "Sourced public-interest research",
    safeguard: "Research draft only",
  },
  "outreach-drafter": {
    name: "Outreach Drafter",
    purpose: "Personalized outreach copy",
    safeguard: "Never sends email",
  },
  "interview-producer": {
    name: "Interview Producer",
    purpose: "Briefs, questions, and run of show",
    safeguard: "Preparation draft only",
  },
  "content-clip-copilot": {
    name: "Content / Clip Copilot",
    purpose: "Transcript-grounded clips and captions",
    safeguard: "Never renders or publishes",
  },
  "operations-briefing-assistant": {
    name: "Operations Briefing Assistant",
    purpose: "Informational operating brief",
    safeguard: "Cannot mutate work",
  },
};

function proposalPersona(proposal: MkAgentProposal): string {
  if (proposal.personaId) return proposal.personaId;
  return proposal.agent
    .toLowerCase()
    .replaceAll(" / ", "-")
    .replaceAll(" ", "-");
}

export function AgentWorkbench({
  personaIds,
  proposals,
  activities,
}: {
  personaIds: MkAgentPersonaId[];
  proposals: MkAgentProposal[];
  activities: MkActivity[];
}) {
  return (
    <section aria-label="Contextual MK Ideas agents">
      <div className="mb-3 flex items-end justify-between gap-4">
        <div>
          <p className="text-2xs font-semibold uppercase tracking-widest text-[#b70f22]">
            Contextual agents
          </p>
          <h2 className="mt-1 font-serif text-2xl font-semibold">
            Drafting bench
          </h2>
        </div>
        <p className="max-w-sm text-right text-xs leading-5 text-muted-foreground">
          Every result preserves its inputs and waits for a human gate.
        </p>
      </div>
      <div className="grid gap-3 lg:grid-cols-2">
        {personaIds.map((personaId) => {
          const copy = PERSONA_COPY[personaId];
          const proposal = proposals.find(
            (item) => proposalPersona(item) === personaId,
          );
          const activity = activities.find(
            (item) => item.personaId === personaId,
          );
          const state =
            activity?.status === "failed"
              ? "failed"
              : proposal
                ? "draft ready"
                : activity?.status || "ready";
          return (
            <article
              className="border border-border bg-card p-4"
              key={personaId}
            >
              <div className="flex items-start justify-between gap-3">
                <div className="flex min-w-0 gap-3">
                  <div className="mt-0.5 border border-border bg-background p-2">
                    <Bot className="h-4 w-4 text-[#b70f22]" />
                  </div>
                  <div className="min-w-0">
                    <h3 className="font-serif text-lg font-semibold">
                      {copy.name}
                    </h3>
                    <p className="mt-1 text-xs text-muted-foreground">
                      {copy.purpose}
                    </p>
                  </div>
                </div>
                <Status>{state}</Status>
              </div>
              <div className="mt-4 flex items-center gap-2 text-2xs font-medium uppercase tracking-wide text-muted-foreground">
                <ShieldCheck className="h-3.5 w-3.5" /> {copy.safeguard}
              </div>
              {proposal ? (
                <p className="mt-3 line-clamp-2 text-xs leading-5 text-muted-foreground">
                  {proposal.summary}
                </p>
              ) : null}
              <div className="mt-4 flex flex-wrap gap-2">
                {proposal ? (
                  <Button
                    onClick={() =>
                      document
                        .getElementById(`proposal-${proposal.proposalId}`)
                        ?.scrollIntoView({
                          behavior: "smooth",
                          block: "center",
                        })
                    }
                    size="sm"
                  >
                    Open draft review
                  </Button>
                ) : null}
                <Button
                  onClick={() => requestOpenCreateAgent()}
                  size="sm"
                  variant={proposal ? "ghost" : "outline"}
                >
                  <ExternalLink className="h-4 w-4" />
                  {proposal ? "Runner settings" : "Configure & launch"}
                </Button>
              </div>
            </article>
          );
        })}
      </div>
    </section>
  );
}
