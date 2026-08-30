import * as React from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import {
  ArrowRight,
  Check,
  Clock3,
  Copy,
  FileText,
  Plus,
  Search,
  Sparkles,
  Upload,
  Users,
  X,
} from "lucide-react";

import { publishApprovalDecision, publishMkState } from "../api";
import { useMkIdeasSnapshot } from "../hooks";
import type { MkAgentProposal, MkRecord } from "../model";
import {
  KIND_MK_CONTENT,
  KIND_MK_INTERVIEW,
  KIND_MK_PERSON,
} from "@/shared/constants/kinds";
import { Button } from "@/shared/ui/button";
import { Input } from "@/shared/ui/input";
import { buildMkIdeasLink } from "@/shared/lib/entityLink";

export type MkIdeasArea = "today" | "work" | "people" | "studio" | "team";

const AREA_COPY: Record<
  MkIdeasArea,
  { eyebrow: string; title: string; note: string }
> = {
  today: {
    eyebrow: "SATURDAY / COMMAND DESK",
    title: "Today",
    note: "The decisions, handoffs, and interviews that need a human eye.",
  },
  work: {
    eyebrow: "OPERATING RHYTHM",
    title: "Work",
    note: "Goals, projects, tasks, meetings, and decisions arrive in V1.",
  },
  people: {
    eyebrow: "RELATIONSHIPS / GUEST PIPELINE",
    title: "People",
    note: "Keep every promising voice moving from first signal to conversation.",
  },
  studio: {
    eyebrow: "INTERVIEW TO PUBLISHABLE IDEA",
    title: "Studio",
    note: "Transcripts, proposed clips, captions, and human review in one place.",
  },
  team: {
    eyebrow: "CONTEXT LIVES WITH THE WORK",
    title: "Team",
    note: "Use Buzz channels and DMs to discuss the records linked from this hub.",
  },
};

function EmptyState({ children }: { children: React.ReactNode }) {
  return (
    <div className="border border-dashed border-border bg-background/50 p-8 text-center text-sm text-muted-foreground">
      {children}
    </div>
  );
}

function Status({ children }: { children: React.ReactNode }) {
  return (
    <span className="inline-flex rounded-full border border-border bg-background px-2 py-0.5 text-2xs font-medium uppercase tracking-wide text-muted-foreground">
      {children}
    </span>
  );
}

function RecordRow({
  record,
  meta,
}: {
  record: MkRecord;
  meta?: React.ReactNode;
}) {
  return (
    <article className="grid gap-3 border-t border-border py-4 first:border-t-0 md:grid-cols-[1fr_auto] md:items-center">
      <div className="min-w-0">
        <div className="mb-1 flex flex-wrap items-center gap-2">
          <h3 className="truncate font-serif text-lg font-semibold text-foreground">
            {record.title}
          </h3>
          <Status>{record.status.replaceAll("_", " ")}</Status>
        </div>
        <p className="text-xs text-muted-foreground">
          v{record.version} · signed by {record.author.slice(0, 8)}… ·{" "}
          {new Date(record.createdAt * 1000).toLocaleDateString()}
        </p>
        <div className="mt-2">
          <TeamReferenceButton
            label={record.title}
            reference={buildMkIdeasLink({
              kind: record.kind as 30803 | 30804 | 30805 | 30809,
              id: record.entityId,
            })}
          />
        </div>
      </div>
      {meta}
    </article>
  );
}

function TeamReferenceButton({
  label,
  reference,
}: {
  label: string;
  reference: string;
}) {
  const [copied, setCopied] = React.useState(false);
  return (
    <button
      className="inline-flex items-center gap-1 text-xs font-medium text-muted-foreground hover:text-foreground"
      onClick={async () => {
        await navigator.clipboard.writeText(`${label} — ${reference}`);
        setCopied(true);
        window.setTimeout(() => setCopied(false), 1600);
      }}
      type="button"
    >
      <Copy className="h-3.5 w-3.5" />
      {copied ? "Copied for Team" : "Copy Team reference"}
    </button>
  );
}

function PageHeader({
  area,
  action,
}: {
  area: MkIdeasArea;
  action?: React.ReactNode;
}) {
  const copy = AREA_COPY[area];
  return (
    <header className="flex flex-col gap-5 border-b border-border pb-7 md:flex-row md:items-end md:justify-between">
      <div>
        <p className="mb-3 text-2xs font-semibold tracking-[0.22em] text-[#b70f22]">
          {copy.eyebrow}
        </p>
        <h1 className="font-serif text-4xl font-semibold tracking-tight text-foreground">
          {copy.title}
        </h1>
        <p className="mt-2 max-w-2xl text-sm leading-6 text-muted-foreground">
          {copy.note}
        </p>
      </div>
      {action}
    </header>
  );
}

function TodayArea({
  records,
  proposals,
  relayUrl,
  queryKey,
}: {
  records: MkRecord[];
  proposals: MkAgentProposal[];
  relayUrl: string;
  queryKey: readonly unknown[];
}) {
  const approvals = records.filter(
    (record) => record.recordType === "approval",
  );
  const reviewed = new Set(
    approvals.map((record) => String(record.data.proposal_id ?? "")),
  );
  const pending = proposals.filter(
    (proposal) => !reviewed.has(proposal.proposalId),
  );
  const interviews = records.filter(
    (record) => record.recordType === "interview",
  );
  const people = records.filter((record) => record.recordType === "person");
  return (
    <div className="space-y-8">
      <PageHeader area="today" />
      <section className="grid gap-4 md:grid-cols-3">
        {[
          [String(pending.length), "Reviews waiting", "Human gate"],
          [String(interviews.length), "Interviews", "In the studio"],
          [
            String(people.length),
            "Guest relationships",
            "Shared across devices",
          ],
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
      <section className="grid gap-8 xl:grid-cols-[1.25fr_0.75fr]">
        <div>
          <div className="mb-3 flex items-center justify-between">
            <h2 className="font-serif text-2xl font-semibold">
              Needs your judgment
            </h2>
            <span className="text-xs text-muted-foreground">
              AI cannot clear this list
            </span>
          </div>
          {pending.length ? (
            pending.map((proposal) => (
              <ProposalCard
                key={proposal.eventId}
                proposal={proposal}
                compact
                queryKey={queryKey}
                relayUrl={relayUrl}
              />
            ))
          ) : (
            <EmptyState>No agent proposals are waiting for review.</EmptyState>
          )}
        </div>
        <div>
          <h2 className="mb-3 font-serif text-2xl font-semibold">Next moves</h2>
          <div className="divide-y divide-border border border-border bg-card px-5">
            {people.slice(0, 4).map((person) => (
              <div
                className="flex items-center gap-3 py-4"
                key={person.entityId}
              >
                <Clock3 className="h-4 w-4 text-[#b70f22]" />
                <div className="min-w-0">
                  <p className="truncate text-sm font-medium">{person.title}</p>
                  <p className="text-xs text-muted-foreground">
                    {person.status.replaceAll("_", " ")}
                  </p>
                </div>
              </div>
            ))}
            {!people.length ? (
              <p className="py-5 text-sm text-muted-foreground">
                Add the first guest in People.
              </p>
            ) : null}
          </div>
        </div>
      </section>
    </div>
  );
}

function PeopleArea({
  records,
  proposals,
  relayUrl,
  queryKey,
}: {
  records: MkRecord[];
  proposals: MkAgentProposal[];
  relayUrl: string;
  queryKey: readonly unknown[];
}) {
  const queryClient = useQueryClient();
  const [adding, setAdding] = React.useState(false);
  const [search, setSearch] = React.useState("");
  const [name, setName] = React.useState("");
  const [organization, setOrganization] = React.useState("");
  const [why, setWhy] = React.useState("");
  const mutation = useMutation({
    mutationFn: () =>
      publishMkState(relayUrl, {
        kind: KIND_MK_PERSON,
        recordType: "person",
        status: "potential",
        fields: {
          name: name.trim(),
          organization: organization.trim(),
          why_now: why.trim(),
          do_not_contact: false,
        },
      }),
    onSuccess: async () => {
      setName("");
      setOrganization("");
      setWhy("");
      setAdding(false);
      await queryClient.invalidateQueries({ queryKey });
    },
  });
  const people = records.filter(
    (record) =>
      record.recordType === "person" &&
      record.title.toLowerCase().includes(search.toLowerCase()),
  );
  return (
    <div className="space-y-7">
      <PageHeader
        area="people"
        action={
          <Button onClick={() => setAdding((value) => !value)}>
            <Plus className="h-4 w-4" /> Add guest
          </Button>
        }
      />
      {adding ? (
        <form
          className="grid gap-4 border border-border bg-card p-5 md:grid-cols-3"
          onSubmit={(event) => {
            event.preventDefault();
            mutation.mutate();
          }}
        >
          <label
            className="grid gap-2 text-xs font-medium"
            htmlFor="mkideas-guest-name"
          >
            Guest name
            <Input
              autoFocus
              id="mkideas-guest-name"
              required
              value={name}
              onChange={(event) => setName(event.target.value)}
            />
          </label>
          <label
            className="grid gap-2 text-xs font-medium"
            htmlFor="mkideas-guest-organization"
          >
            Organization
            <Input
              id="mkideas-guest-organization"
              value={organization}
              onChange={(event) => setOrganization(event.target.value)}
            />
          </label>
          <label
            className="grid gap-2 text-xs font-medium"
            htmlFor="mkideas-guest-why"
          >
            Why now?
            <Input
              id="mkideas-guest-why"
              value={why}
              onChange={(event) => setWhy(event.target.value)}
            />
          </label>
          <div className="flex items-center gap-2 md:col-span-3">
            <Button disabled={!name.trim() || mutation.isPending} type="submit">
              Save human-signed guest
            </Button>
            <Button
              onClick={() => setAdding(false)}
              type="button"
              variant="ghost"
            >
              Cancel
            </Button>
            {mutation.error ? (
              <span className="text-xs text-destructive">
                {mutation.error.message}
              </span>
            ) : null}
          </div>
        </form>
      ) : null}
      <div className="flex max-w-md items-center gap-2 border-b border-border pb-2">
        <Search className="h-4 w-4 text-muted-foreground" />
        <input
          className="w-full bg-transparent text-sm outline-none"
          onChange={(event) => setSearch(event.target.value)}
          placeholder="Filter people"
          value={search}
        />
      </div>
      <section className="border-y border-border">
        {people.length ? (
          people.map((person) => {
            const research = proposals.filter(
              (proposal) =>
                proposal.targetKind === KIND_MK_PERSON &&
                proposal.targetId === person.entityId,
            );
            return (
              <div
                className="border-t border-border first:border-t-0"
                key={person.entityId}
              >
                <RecordRow
                  record={person}
                  meta={
                    <div className="text-right">
                      <p className="text-xs font-medium">
                        {String(person.data.organization ?? "Independent")}
                      </p>
                      <p className="mt-1 text-xs text-muted-foreground">
                        {String(person.data.why_now ?? "Research next")}
                      </p>
                    </div>
                  }
                />
                {research.map((proposal) => (
                  <div
                    className="mb-4 border-l-2 border-[#b70f22] bg-card px-4 py-3"
                    key={proposal.eventId}
                  >
                    <p className="text-2xs font-semibold uppercase tracking-widest text-[#b70f22]">
                      Attached research · {proposal.agent}
                    </p>
                    <p className="mt-1 text-sm leading-6">{proposal.summary}</p>
                    <p className="mt-2 text-xs text-muted-foreground">
                      {proposal.provenance.length} provenance source
                      {proposal.provenance.length === 1 ? "" : "s"} · proposal
                      v1
                    </p>
                  </div>
                ))}
              </div>
            );
          })
        ) : (
          <EmptyState>No guests match this view.</EmptyState>
        )}
      </section>
    </div>
  );
}

function ProposalCard({
  proposal,
  relayUrl,
  queryKey,
  compact = false,
  reviewedDecision,
}: {
  proposal: MkAgentProposal;
  relayUrl: string;
  queryKey: readonly unknown[];
  compact?: boolean;
  reviewedDecision?: string;
}) {
  const queryClient = useQueryClient();
  const decision = useMutation({
    mutationFn: (value: "approved" | "rejected") =>
      publishApprovalDecision(
        relayUrl,
        proposal.eventId,
        proposal.proposalId,
        proposal.targetId,
        proposal.targetKind,
        value,
      ),
    onSuccess: () => queryClient.invalidateQueries({ queryKey }),
  });
  return (
    <article className="mb-3 border border-border bg-card p-5">
      <div className="flex items-start justify-between gap-4">
        <div>
          <p className="text-2xs font-semibold uppercase tracking-widest text-[#b70f22]">
            {proposal.agent || "MK agent"} · proposed
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
      <div className="mt-4 flex flex-wrap items-center gap-2">
        {reviewedDecision ? (
          <Status>Human review · {reviewedDecision}</Status>
        ) : (
          <>
            <Button
              disabled={decision.isPending}
              onClick={() => decision.mutate("approved")}
              size="sm"
            >
              <Check className="h-4 w-4" /> Approve
            </Button>
            <Button
              disabled={decision.isPending}
              onClick={() => decision.mutate("rejected")}
              size="sm"
              variant="outline"
            >
              <X className="h-4 w-4" /> Reject
            </Button>
            <span className="text-xs text-muted-foreground">
              Signed review required
            </span>
          </>
        )}
        <TeamReferenceButton
          label={proposal.summary}
          reference={buildMkIdeasLink({
            kind: proposal.targetKind as 30803 | 30804 | 30805,
            id: proposal.targetId,
            proposalId: proposal.proposalId,
          })}
        />
      </div>
    </article>
  );
}

function StudioArea({
  records,
  proposals,
  relayUrl,
  queryKey,
}: {
  records: MkRecord[];
  proposals: MkAgentProposal[];
  relayUrl: string;
  queryKey: readonly unknown[];
}) {
  const queryClient = useQueryClient();
  const people = records.filter((record) => record.recordType === "person");
  const interviews = records.filter(
    (record) => record.recordType === "interview",
  );
  const contents = records.filter((record) => record.recordType === "content");
  const approvalDecisions = new Map(
    records
      .filter((record) => record.recordType === "approval")
      .map((record) => [String(record.data.proposal_id ?? ""), record.status]),
  );
  const studioProposals = proposals.filter(
    (proposal) =>
      proposal.targetKind === KIND_MK_INTERVIEW ||
      proposal.targetKind === KIND_MK_CONTENT,
  );
  const [guestId, setGuestId] = React.useState(people[0]?.entityId ?? "");
  const createInterview = useMutation({
    mutationFn: () =>
      publishMkState(relayUrl, {
        kind: KIND_MK_INTERVIEW,
        recordType: "interview",
        status: "planning",
        fields: {
          title: `Interview with ${people.find((item) => item.entityId === guestId)?.title ?? "guest"}`,
          guest_id: guestId,
        },
      }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey }),
  });
  const uploadTranscript = async (record: MkRecord, file: File) => {
    const transcript = await file.text();
    await publishMkState(relayUrl, {
      kind: KIND_MK_INTERVIEW,
      recordType: "interview",
      previous: record,
      status: "content_processing",
      fields: {
        ...record.data,
        transcript_name: file.name,
        transcript_text: transcript,
      },
    });
    await queryClient.invalidateQueries({ queryKey });
  };
  const createContent = async (interview: MkRecord) => {
    await publishMkState(relayUrl, {
      kind: KIND_MK_CONTENT,
      recordType: "content",
      status: "internal_review",
      fields: {
        title: `${interview.title} — clips`,
        interview_id: interview.entityId,
      },
    });
    await queryClient.invalidateQueries({ queryKey });
  };
  return (
    <div className="space-y-8">
      <PageHeader area="studio" />
      <section>
        <div className="mb-3 flex flex-wrap items-center justify-between gap-3">
          <h2 className="font-serif text-2xl font-semibold">Interviews</h2>
          <div className="flex items-center gap-2">
            <select
              className="h-9 border border-border bg-background px-3 text-sm"
              onChange={(event) => setGuestId(event.target.value)}
              value={guestId}
            >
              <option value="">Choose a guest</option>
              {people.map((person) => (
                <option key={person.entityId} value={person.entityId}>
                  {person.title}
                </option>
              ))}
            </select>
            <Button
              disabled={!guestId || createInterview.isPending}
              onClick={() => createInterview.mutate()}
              size="sm"
            >
              <Plus className="h-4 w-4" /> Interview
            </Button>
          </div>
        </div>
        <div className="border-y border-border">
          {interviews.length ? (
            interviews.map((interview) => (
              <RecordRow
                key={interview.entityId}
                record={interview}
                meta={
                  <div className="flex flex-wrap justify-end gap-2">
                    <label className="inline-flex h-8 cursor-pointer items-center gap-2 border border-border px-3 text-xs font-medium">
                      <Upload className="h-3.5 w-3.5" /> Transcript
                      <input
                        accept=".txt,.vtt,.srt"
                        className="sr-only"
                        onChange={(event) => {
                          const file = event.target.files?.[0];
                          if (file) void uploadTranscript(interview, file);
                        }}
                        type="file"
                      />
                    </label>
                    <Button
                      onClick={() => void createContent(interview)}
                      size="sm"
                      variant="outline"
                    >
                      <FileText className="h-4 w-4" /> Content record
                    </Button>
                  </div>
                }
              />
            ))
          ) : (
            <EmptyState>Create an interview after adding a guest.</EmptyState>
          )}
        </div>
      </section>
      <section>
        <div className="mb-3 flex items-center justify-between">
          <h2 className="font-serif text-2xl font-semibold">
            Clip & caption desk
          </h2>
          <span className="text-xs text-muted-foreground">
            {contents.length} content records
          </span>
        </div>
        {studioProposals.length ? (
          studioProposals.map((proposal) => (
            <ProposalCard
              key={proposal.eventId}
              proposal={proposal}
              queryKey={queryKey}
              relayUrl={relayUrl}
              reviewedDecision={approvalDecisions.get(proposal.proposalId)}
            />
          ))
        ) : (
          <EmptyState>
            The Content / Clip Copilot’s timestamped proposals will appear here.
            Upload a transcript, then run the managed agent.
          </EmptyState>
        )}
      </section>
    </div>
  );
}

export function MkIdeasWorkspace({ area }: { area: MkIdeasArea }) {
  const { query, queryKey, relayUrl } = useMkIdeasSnapshot();
  const snapshot = query.data ?? { records: [], proposals: [] };
  if (!relayUrl)
    return (
      <EmptyState>
        Connect to the private MK Ideas community to continue.
      </EmptyState>
    );
  return (
    <main
      className="h-full overflow-y-auto bg-[#fbfaf7] px-6 py-8 text-[#141415] dark:bg-background dark:text-foreground lg:px-10"
      data-testid={`mkideas-${area}`}
    >
      <div className="mx-auto max-w-6xl">
        {query.isLoading ? (
          <p className="text-sm text-muted-foreground">
            Loading shared MK Ideas state…
          </p>
        ) : null}
        {query.error ? (
          <p className="mb-4 border border-destructive/30 bg-destructive/5 p-3 text-sm text-destructive">
            {query.error.message}
          </p>
        ) : null}
        {area === "today" ? (
          <TodayArea
            proposals={snapshot.proposals}
            queryKey={queryKey}
            records={snapshot.records}
            relayUrl={relayUrl}
          />
        ) : null}
        {area === "people" ? (
          <PeopleArea
            proposals={snapshot.proposals}
            queryKey={queryKey}
            records={snapshot.records}
            relayUrl={relayUrl}
          />
        ) : null}
        {area === "studio" ? (
          <StudioArea
            proposals={snapshot.proposals}
            queryKey={queryKey}
            records={snapshot.records}
            relayUrl={relayUrl}
          />
        ) : null}
        {area === "work" ? (
          <div className="space-y-7">
            <PageHeader area="work" />
            <EmptyState>
              <div className="mx-auto max-w-lg">
                <Users className="mx-auto mb-3 h-6 w-6" />
                <p className="font-medium text-foreground">
                  Deliberately quiet in V0
                </p>
                <p className="mt-2">
                  The architecture reserves goals, operational projects, tasks,
                  meetings, and decisions. Their production workflows are part
                  of V1.
                </p>
              </div>
            </EmptyState>
          </div>
        ) : null}
        {area === "team" ? (
          <div className="space-y-7">
            <PageHeader area="team" />
            <EmptyState>
              <div className="mx-auto max-w-lg">
                <ArrowRight className="mx-auto mb-3 h-6 w-6" />
                <p className="font-medium text-foreground">
                  Choose a channel or direct message
                </p>
                <p className="mt-2">
                  Team remains Buzz-native. Link a guest, interview, content
                  item, or proposal in the conversation where the decision
                  happens.
                </p>
              </div>
            </EmptyState>
          </div>
        ) : null}
      </div>
    </main>
  );
}
