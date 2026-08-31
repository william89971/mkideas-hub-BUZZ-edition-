import { Ban, Mail, MapPin, UserRound } from "lucide-react";

import type { MkActivity, MkAgentProposal, MkRecord } from "../model";
import { KIND_MK_PERSON } from "@/shared/constants/kinds";
import { EntityHistoryTimeline } from "./EntityHistoryTimeline";
import { ProposalCard } from "./ProposalCard";
import { Status, TeamReferenceButton } from "./MkIdeasPrimitives";
import { buildMkIdeasLink } from "@/shared/lib/entityLink";

function textList(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((item): item is string => typeof item === "string")
    : typeof value === "string" && value
      ? [value]
      : [];
}

export function PersonProfile({
  person,
  records,
  proposals,
  activities,
  relayUrl,
  community,
  queryKey,
}: {
  person: MkRecord;
  records: MkRecord[];
  proposals: MkAgentProposal[];
  activities: MkActivity[];
  relayUrl: string;
  community: string;
  queryKey: readonly unknown[];
}) {
  const personProposals = proposals.filter(
    (proposal) =>
      proposal.targetKind === KIND_MK_PERSON &&
      proposal.targetId === person.entityId,
  );
  const related = records.filter(
    (record) =>
      record.entityId !== person.entityId &&
      (record.data.guest_id === person.entityId ||
        record.data.person_id === person.entityId),
  );
  const personActivities = activities.filter(
    (activity) =>
      activity.targetKind === KIND_MK_PERSON &&
      activity.targetId === person.entityId,
  );
  const timeline = [
    {
      key: person.eventId,
      at: person.createdAt,
      label: `Profile moved to ${person.status.replaceAll("_", " ")}`,
      note: `Human-signed revision v${person.version}`,
    },
    ...personProposals.map((proposal) => ({
      key: proposal.eventId,
      at: proposal.createdAt,
      label: `${proposal.agent} produced ${proposal.proposalType.replaceAll("_", " ")}`,
      note: `${proposal.provenance.length} sourced inputs · draft only`,
    })),
    ...personActivities.map((activity) => ({
      key: activity.eventId,
      at: activity.createdAt,
      label: activity.summary || activity.activityType.replaceAll("_", " "),
      note: activity.status || "activity",
    })),
    ...related.map((record) => ({
      key: record.eventId,
      at: record.createdAt,
      label: `${record.recordType}: ${record.title}`,
      note: record.status,
    })),
  ].sort((a, b) => b.at - a.at);
  const dnc = Boolean(person.data.do_not_contact);
  const topics = textList(person.data.topics ?? person.data.tags);
  return (
    <div className="space-y-6 border border-border bg-card p-6">
      <header className="flex flex-col gap-4 border-b border-border pb-5 md:flex-row md:items-start md:justify-between">
        <div>
          <p className="text-2xs font-semibold uppercase tracking-widest text-[#b70f22]">
            Relationship profile
          </p>
          <h2 className="mt-2 font-serif text-3xl font-semibold">
            {person.title}
          </h2>
          <p className="mt-2 text-sm text-muted-foreground">
            {String(person.data.title ?? "Guest prospect")} ·{" "}
            {String(person.data.organization ?? "Independent")}
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <Status>{person.status.replaceAll("_", " ")}</Status>
          {dnc ? <Status>do not contact</Status> : null}
        </div>
      </header>

      {dnc ? (
        <div className="flex gap-3 border border-[#b70f22]/30 bg-[#b70f22]/5 p-4">
          <Ban className="mt-0.5 h-4 w-4 shrink-0 text-[#b70f22]" />
          <div>
            <p className="text-sm font-medium">Outreach is blocked.</p>
            <p className="mt-1 text-xs leading-5 text-muted-foreground">
              Only the owner may clear DNC, with a signed reason and audit
              evidence.
            </p>
          </div>
        </div>
      ) : null}

      <div className="grid gap-5 md:grid-cols-2 xl:grid-cols-4">
        {[
          [
            UserRound,
            "Owner",
            String(person.data.owner ?? person.data.assignee ?? "Unassigned"),
          ],
          [Mail, "Contact", String(person.data.email ?? "No email recorded")],
          [MapPin, "Location", String(person.data.location ?? "Not recorded")],
          [
            UserRound,
            "Why now",
            String(person.data.why_now ?? "No rationale recorded"),
          ],
        ].map(([Icon, label, value]) => {
          const DetailIcon = Icon as typeof UserRound;
          return (
            <div key={String(label)}>
              <p className="flex items-center gap-2 text-2xs font-semibold uppercase tracking-widest text-muted-foreground">
                <DetailIcon className="h-3.5 w-3.5" /> {String(label)}
              </p>
              <p className="mt-2 text-sm leading-5">{String(value)}</p>
            </div>
          );
        })}
      </div>

      {topics.length ? (
        <div className="flex flex-wrap gap-2">
          {topics.map((topic) => (
            <Status key={topic}>{topic}</Status>
          ))}
        </div>
      ) : null}

      <section>
        <div className="mb-3 flex items-end justify-between">
          <h3 className="font-serif text-xl font-semibold">
            Research & outreach drafts
          </h3>
          <span className="text-xs text-muted-foreground">
            Sending disabled
          </span>
        </div>
        {personProposals.map((proposal) => (
          <ProposalCard
            community={community}
            key={proposal.eventId}
            proposal={proposal}
            queryKey={queryKey}
            relayUrl={relayUrl}
          />
        ))}
      </section>

      <section className="grid gap-6 lg:grid-cols-[1fr_0.75fr]">
        <div>
          <h3 className="mb-3 font-serif text-xl font-semibold">
            Relationship timeline
          </h3>
          <div className="border-l border-border pl-4">
            {timeline.map((item) => (
              <div className="relative pb-4 last:pb-0" key={item.key}>
                <span className="absolute -left-[1.17rem] top-1.5 h-2 w-2 rounded-full bg-[#b70f22]" />
                <p className="text-sm font-medium">{item.label}</p>
                <p className="mt-1 text-xs text-muted-foreground">
                  {new Date(item.at * 1000).toLocaleString()} · {item.note}
                </p>
              </div>
            ))}
          </div>
        </div>
        <div>
          <h3 className="mb-3 font-serif text-xl font-semibold">Linked work</h3>
          {related.length ? (
            related.map((record) => (
              <div
                className="border-t border-border py-3 first:border-t-0"
                key={record.eventId}
              >
                <p className="text-sm font-medium">{record.title}</p>
                <p className="mt-1 text-xs text-muted-foreground">
                  {record.recordType} · {record.status}
                </p>
              </div>
            ))
          ) : (
            <p className="text-sm text-muted-foreground">
              No linked interviews or content yet.
            </p>
          )}
          <div className="mt-4">
            <TeamReferenceButton
              label={person.title}
              reference={buildMkIdeasLink({
                community,
                kind: person.kind,
                id: person.entityId,
              })}
            />
          </div>
        </div>
      </section>
      <EntityHistoryTimeline record={person} relayUrl={relayUrl} />
    </div>
  );
}
