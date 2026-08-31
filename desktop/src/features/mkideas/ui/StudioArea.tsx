import * as React from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { FileText, Plus, Upload } from "lucide-react";

import { publishMkState } from "../api";
import type {
  MkActivity,
  MkAgentProposal,
  MkApprovalAction,
  MkRecord,
} from "../model";
import { KIND_MK_CONTENT, KIND_MK_INTERVIEW } from "@/shared/constants/kinds";
import { pickAndUploadMedia } from "@/shared/api/tauri";
import { Button } from "@/shared/ui/button";
import { AgentWorkbench } from "./AgentWorkbench";
import { EmptyState, PageHeader, Status } from "./MkIdeasPrimitives";
import { ProposalCard } from "./ProposalCard";
import { StudioRecordDetail } from "./StudioRecordDetail";

export function StudioArea({
  records,
  proposals,
  approvalActions,
  activities,
  relayUrl,
  community,
  queryKey,
}: {
  records: MkRecord[];
  proposals: MkAgentProposal[];
  approvalActions: MkApprovalAction[];
  activities: MkActivity[];
  relayUrl: string;
  community: string;
  queryKey: readonly unknown[];
}) {
  const queryClient = useQueryClient();
  const people = records.filter((record) => record.recordType === "person");
  const interviews = records.filter(
    (record) => record.recordType === "interview",
  );
  const contents = records.filter((record) => record.recordType === "content");
  const approvals = new Map(
    records
      .filter((record) => record.recordType === "approval")
      .map((record) => [String(record.data.proposal_id ?? ""), record]),
  );
  const reviewActions = new Map(
    approvalActions.map((action) => [action.proposalId, action.decision]),
  );
  const studioProposals = proposals.filter(
    (proposal) =>
      proposal.targetKind === KIND_MK_INTERVIEW ||
      proposal.targetKind === KIND_MK_CONTENT,
  );
  const [guestId, setGuestId] = React.useState(people[0]?.entityId ?? "");
  const [selectedInterviewId, setSelectedInterviewId] = React.useState("");
  const selectedInterview =
    interviews.find((item) => item.entityId === selectedInterviewId) ??
    interviews[0];
  React.useEffect(() => {
    if (!guestId && people[0]) setGuestId(people[0].entityId);
  }, [guestId, people]);
  const createInterview = useMutation({
    mutationFn: () =>
      publishMkState(relayUrl, {
        kind: KIND_MK_INTERVIEW,
        recordType: "interview",
        status: "planning",
        fields: {
          title: `Interview with ${people.find((item) => item.entityId === guestId)?.title ?? "guest"}`,
          guest_id: guestId,
          questions: [],
        },
      }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey }),
  });
  const uploadTranscript = async (record: MkRecord) => {
    const [media] = await pickAndUploadMedia(
      `mk-transcript-${record.entityId}`,
    );
    if (!media) return;
    const filename = media.filename ?? "transcript";
    const format = filename.split(".").at(-1)?.toLowerCase() ?? "text";
    await publishMkState(relayUrl, {
      kind: KIND_MK_INTERVIEW,
      recordType: "interview",
      previous: record,
      status: "transcribing",
      fields: {
        ...record.data,
        transcript_descriptor: {
          media_id: media.sha256,
          object_key: media.url,
          sha256: media.sha256,
          mime_type: media.type,
          size: media.size,
          original_filename: filename,
          uploaded_at: new Date(media.uploaded * 1000).toISOString(),
          format,
          duration: media.duration ?? null,
          language: "und",
          version:
            Number(
              (
                record.data.transcript_descriptor as
                  | { version?: number }
                  | undefined
              )?.version ?? 0,
            ) + 1,
          source: "private_media_upload",
          provenance: { type: "human_upload", ref: media.sha256 },
        },
      },
    });
    await queryClient.invalidateQueries({ queryKey });
  };
  const createContent = async (interview: MkRecord) => {
    await publishMkState(relayUrl, {
      kind: KIND_MK_CONTENT,
      recordType: "content",
      status: "in-review",
      fields: {
        title: `${interview.title} — editorial package`,
        interview_id: interview.entityId,
        publication_state: "not_published",
      },
    });
    await queryClient.invalidateQueries({ queryKey });
  };
  return (
    <div className="space-y-8">
      <PageHeader area="studio" />
      <section className="grid gap-6 xl:grid-cols-[18rem_1fr]">
        <div>
          <div className="mb-3 flex flex-col gap-2">
            <h2 className="font-serif text-2xl font-semibold">Interviews</h2>
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
              <Plus className="h-4 w-4" /> New interview
            </Button>
          </div>
          <div className="border-y border-border">
            {interviews.map((interview) => (
              <button
                className={`w-full border-t border-border px-3 py-4 text-left first:border-t-0 ${selectedInterview?.entityId === interview.entityId ? "bg-card" : "hover:bg-card/50"}`}
                key={interview.entityId}
                onClick={() => setSelectedInterviewId(interview.entityId)}
                type="button"
              >
                <p className="font-serif text-base font-semibold">
                  {interview.title}
                </p>
                <div className="mt-2">
                  <Status>{interview.status}</Status>
                </div>
              </button>
            ))}
            {!interviews.length ? (
              <EmptyState>Create an interview after adding a guest.</EmptyState>
            ) : null}
          </div>
          {selectedInterview ? (
            <div className="mt-3 grid gap-2">
              <Button
                onClick={() => void uploadTranscript(selectedInterview)}
                size="sm"
                variant="outline"
              >
                <Upload className="h-4 w-4" /> Upload transcript
              </Button>
              <Button
                onClick={() => void createContent(selectedInterview)}
                size="sm"
                variant="outline"
              >
                <FileText className="h-4 w-4" /> Create content record
              </Button>
            </div>
          ) : null}
        </div>
        {selectedInterview ? (
          <StudioRecordDetail
            contents={contents}
            interview={selectedInterview}
            relayUrl={relayUrl}
          />
        ) : (
          <EmptyState>
            Select an interview to open the studio record.
          </EmptyState>
        )}
      </section>

      <section>
        <div className="mb-3 flex items-end justify-between gap-4">
          <div>
            <p className="text-2xs font-semibold uppercase tracking-widest text-[#b70f22]">
              Human review queue
            </p>
            <h2 className="mt-1 font-serif text-2xl font-semibold">
              Clips, captions & interview drafts
            </h2>
          </div>
          <span className="text-xs text-muted-foreground">
            No rendering · no publishing
          </span>
        </div>
        {studioProposals.length ? (
          studioProposals.map((proposal) => (
            <ProposalCard
              approval={
                approvals.get(proposal.proposalId) as
                  | MkRecord<"approval">
                  | undefined
              }
              community={community}
              key={proposal.eventId}
              proposal={proposal}
              queryKey={queryKey}
              relayUrl={relayUrl}
              reviewedDecision={reviewActions.get(proposal.proposalId)}
            />
          ))
        ) : (
          <EmptyState>
            Timestamped and interview-preparation drafts will appear after agent
            runs.
          </EmptyState>
        )}
      </section>

      <AgentWorkbench
        activities={activities}
        personaIds={["interview-producer", "content-clip-copilot"]}
        proposals={proposals}
      />
    </div>
  );
}
