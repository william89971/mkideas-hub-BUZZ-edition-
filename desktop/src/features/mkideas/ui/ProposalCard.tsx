import * as React from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Check, Sparkles, X } from "lucide-react";

import { publishApprovalDecision } from "../api";
import { isMkStateKind, type MkAgentProposal, type MkRecord } from "../model";
import { Button } from "@/shared/ui/button";
import { buildMkIdeasLink } from "@/shared/lib/entityLink";
import { Status, TeamReferenceButton } from "./MkIdeasPrimitives";

export function ProposalCard({
  proposal,
  relayUrl,
  community,
  queryKey,
  compact = false,
  reviewedDecision,
  approval,
}: {
  proposal: MkAgentProposal;
  relayUrl: string;
  community: string;
  queryKey: readonly unknown[];
  compact?: boolean;
  reviewedDecision?: string;
  approval?: MkRecord<"approval">;
}) {
  const queryClient = useQueryClient();
  const [reason, setReason] = React.useState("");
  const decision = useMutation({
    mutationFn: (value: "approved" | "rejected") =>
      approval
        ? publishApprovalDecision(relayUrl, {
            approval,
            proposal,
            decision: value,
            reason,
          })
        : Promise.reject(new Error("A signed approval request is required.")),
    onSuccess: () => queryClient.invalidateQueries({ queryKey }),
  });
  return (
    <article
      className="mb-3 border border-border bg-card p-5"
      id={`proposal-${proposal.proposalId}`}
    >
      <div className="flex items-start justify-between gap-4">
        <div>
          <p className="text-2xs font-semibold uppercase tracking-widest text-[#b70f22]">
            {proposal.agent || proposal.personaId || "MK agent"} ·{" "}
            {proposal.status || "proposed"}
          </p>
          <h3 className="mt-2 font-serif text-xl font-semibold">
            {proposal.summary}
          </h3>
        </div>
        <Sparkles className="h-5 w-5 text-[#b70f22]" />
      </div>
      {!compact && proposal.clips.length ? (
        <div className="mt-4 divide-y divide-border border-y border-border">
          {proposal.clips.map((clip) => (
            <div
              className="grid gap-2 py-3 md:grid-cols-[7rem_1fr]"
              key={`${clip.start}-${clip.title}`}
            >
              <span className="font-mono text-xs text-muted-foreground">
                {clip.start}—{clip.end}
              </span>
              <div>
                <p className="text-sm font-medium">{clip.title}</p>
                <p className="mt-1 text-xs leading-5 text-muted-foreground">
                  {clip.caption}
                </p>
              </div>
            </div>
          ))}
        </div>
      ) : null}
      {!compact && Object.keys(proposal.output).length ? (
        <div className="mt-4 border-l-2 border-[#b70f22] bg-background px-4 py-3">
          {typeof proposal.output.subject === "string" ? (
            <p className="text-sm font-medium">{proposal.output.subject}</p>
          ) : null}
          {typeof proposal.output.body === "string" ? (
            <p className="mt-2 whitespace-pre-line text-xs leading-5 text-muted-foreground">
              {proposal.output.body}
            </p>
          ) : null}
          {Array.isArray(proposal.output.questions) ? (
            <ul className="mt-2 list-disc space-y-1 pl-4 text-xs text-muted-foreground">
              {proposal.output.questions.slice(0, 4).map((question) => (
                <li key={String(question)}>{String(question)}</li>
              ))}
            </ul>
          ) : null}
        </div>
      ) : null}
      <div className="mt-4 flex flex-wrap gap-x-4 gap-y-1 text-2xs text-muted-foreground">
        <span>proposal v{proposal.proposalVersion}</span>
        <span>{proposal.provenance.length} sources</span>
        {proposal.templateVersion ? (
          <span>{proposal.templateVersion}</span>
        ) : null}
        {proposal.provider ? (
          <span>
            {proposal.provider} / {proposal.model || "model n/a"}
          </span>
        ) : null}
      </div>
      <div className="mt-4 flex flex-wrap items-center gap-2">
        {reviewedDecision ? (
          <Status>Human review · {reviewedDecision}</Status>
        ) : !approval ? (
          <Status>Draft only · awaiting signed approval request</Status>
        ) : (
          <div className="w-full border-t border-border pt-4">
            <label className="grid gap-2 text-xs font-medium">
              Review reason
              <textarea
                className="min-h-20 w-full resize-y border border-border bg-background px-3 py-2 text-sm outline-none focus:border-[#b70f22]"
                onChange={(event) => setReason(event.target.value)}
                placeholder="Record the human judgment behind this decision."
                value={reason}
              />
            </label>
            <div className="mt-3 flex flex-wrap items-center gap-2">
              <Button
                disabled={decision.isPending || !reason.trim()}
                onClick={() => decision.mutate("approved")}
                size="sm"
              >
                <Check className="h-4 w-4" /> Approve
              </Button>
              <Button
                disabled={decision.isPending || !reason.trim()}
                onClick={() => decision.mutate("rejected")}
                size="sm"
                variant="outline"
              >
                <X className="h-4 w-4" /> Reject
              </Button>
              <span className="text-xs text-muted-foreground">
                One signed action · relay-atomic decision
              </span>
            </div>
          </div>
        )}
        {isMkStateKind(proposal.targetKind) ? (
          <TeamReferenceButton
            label={proposal.summary}
            reference={buildMkIdeasLink({
              community,
              kind: proposal.targetKind,
              id: proposal.targetId,
              proposalId: proposal.proposalId,
            })}
          />
        ) : null}
      </div>
      {decision.error ? (
        <p className="mt-3 text-xs text-destructive">
          {decision.error.message}
        </p>
      ) : null}
    </article>
  );
}
