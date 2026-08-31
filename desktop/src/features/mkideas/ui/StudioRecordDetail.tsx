import * as React from "react";
import { FileAudio, FileText, Search, ShieldCheck } from "lucide-react";

import type { MkRecord } from "../model";
import { EntityHistoryTimeline } from "./EntityHistoryTimeline";
import { Status } from "./MkIdeasPrimitives";

function object(value: unknown): Record<string, unknown> {
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};
}

function stringList(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((item): item is string => typeof item === "string")
    : [];
}

export function StudioRecordDetail({
  interview,
  contents,
  relayUrl,
}: {
  interview: MkRecord;
  contents: MkRecord[];
  relayUrl: string;
}) {
  const descriptor = object(
    interview.data.transcript_descriptor ?? interview.data.transcript,
  );
  const segments = Array.isArray(interview.data.transcript_segments)
    ? interview.data.transcript_segments.flatMap((value) =>
        value && typeof value === "object" && !Array.isArray(value)
          ? [value as Record<string, unknown>]
          : [],
      )
    : [];
  const [query, setQuery] = React.useState("");
  const [activeTimestamp, setActiveTimestamp] = React.useState("");
  const visibleSegments = segments.filter((segment) =>
    String(segment.text ?? "")
      .toLowerCase()
      .includes(query.toLowerCase()),
  );
  const questions = stringList(interview.data.questions);
  const linkedContent = contents.filter(
    (record) => record.data.interview_id === interview.entityId,
  );
  return (
    <div
      className="space-y-6 border border-border bg-card p-6"
      data-testid="mkideas-studio-record-detail"
    >
      <header className="flex flex-col gap-4 border-b border-border pb-5 md:flex-row md:items-start md:justify-between">
        <div>
          <p className="text-2xs font-semibold uppercase tracking-widest text-[#b70f22]">
            Interview desk
          </p>
          <h2 className="mt-2 font-serif text-3xl font-semibold">
            {interview.title}
          </h2>
          <p className="mt-2 text-xs text-muted-foreground">
            v{interview.version} · recording, transcript, preparation, and
            linked content
          </p>
        </div>
        <Status>{interview.status}</Status>
      </header>

      <section className="grid gap-6 lg:grid-cols-2">
        <div>
          <h3 className="font-serif text-xl font-semibold">Preparation</h3>
          <p className="mt-2 text-sm leading-6 text-muted-foreground">
            {String(interview.data.brief ?? "No producer brief attached yet.")}
          </p>
          {questions.length ? (
            <ol className="mt-4 list-decimal space-y-2 pl-5 text-sm">
              {questions.map((question) => (
                <li key={question}>{question}</li>
              ))}
            </ol>
          ) : null}
        </div>
        <div className="border border-border bg-background p-4">
          <div className="flex items-start justify-between gap-3">
            <div className="flex gap-3">
              <FileAudio className="mt-1 h-4 w-4 text-[#b70f22]" />
              <div>
                <p className="text-sm font-medium">
                  {String(
                    descriptor.original_filename ??
                      descriptor.filename ??
                      "No transcript uploaded",
                  )}
                </p>
                <p className="mt-1 text-xs text-muted-foreground">
                  {String(descriptor.format ?? "—")} ·{" "}
                  {String(descriptor.language ?? "language n/a")} · v
                  {String(descriptor.version ?? "—")}
                </p>
              </div>
            </div>
            <Status>private media</Status>
          </div>
          {descriptor.sha256 ? (
            <p className="mt-4 break-all font-mono text-2xs text-muted-foreground">
              sha256 {String(descriptor.sha256)}
            </p>
          ) : null}
          <p className="mt-3 flex items-center gap-2 text-2xs text-muted-foreground">
            <ShieldCheck className="h-3.5 w-3.5" /> Full transcript bytes stay
            outside Nostr events.
          </p>
        </div>
      </section>

      <section>
        <div className="mb-3 flex flex-col gap-3 md:flex-row md:items-end md:justify-between">
          <div>
            <p className="text-2xs font-semibold uppercase tracking-widest text-[#b70f22]">
              Authorized transcript index
            </p>
            <h3 className="mt-1 font-serif text-xl font-semibold">
              Timestamp navigator
            </h3>
          </div>
          <label className="flex items-center gap-2 border-b border-border pb-2 text-xs">
            <Search className="h-3.5 w-3.5 text-muted-foreground" />
            <input
              className="bg-transparent outline-none"
              onChange={(event) => setQuery(event.target.value)}
              placeholder="Search this transcript"
              value={query}
            />
          </label>
        </div>
        <div className="divide-y divide-border border-y border-border">
          {visibleSegments.map((segment) => {
            const start = String(segment.start ?? segment.timestamp ?? "—");
            return (
              <button
                className={`grid w-full gap-2 px-2 py-3 text-left md:grid-cols-[7rem_1fr] ${activeTimestamp === start ? "bg-[#b70f22]/5" : "hover:bg-background"}`}
                key={`${start}-${String(segment.text ?? "")}`}
                onClick={() => setActiveTimestamp(start)}
                type="button"
              >
                <span className="font-mono text-xs text-[#b70f22]">
                  {start}
                </span>
                <span className="text-sm leading-6">
                  {String(segment.text ?? "")}
                </span>
              </button>
            );
          })}
          {!visibleSegments.length ? (
            <p className="py-5 text-sm text-muted-foreground">
              {segments.length
                ? "No transcript segments match."
                : "Derived transcript segments will appear after indexing."}
            </p>
          ) : null}
        </div>
      </section>

      <section>
        <h3 className="mb-3 font-serif text-xl font-semibold">
          Linked content versions
        </h3>
        {linkedContent.length ? (
          linkedContent.map((content) => (
            <div
              className="grid gap-3 border-t border-border py-3 first:border-t-0 md:grid-cols-[1fr_auto]"
              key={content.eventId}
            >
              <div className="flex gap-3">
                <FileText className="mt-1 h-4 w-4 text-[#b70f22]" />
                <div>
                  <p className="text-sm font-medium">{content.title}</p>
                  <p className="mt-1 text-xs text-muted-foreground">
                    v{content.version} · {content.status}
                  </p>
                </div>
              </div>
              <Status>
                {content.status === "published" ? "published" : "not published"}
              </Status>
            </div>
          ))
        ) : (
          <p className="text-sm text-muted-foreground">
            No linked content record yet.
          </p>
        )}
        <p className="mt-3 text-2xs font-medium uppercase tracking-wide text-muted-foreground">
          Publishing and video rendering are disabled in MK Ideas Buzz.
        </p>
      </section>
      <EntityHistoryTimeline record={interview} relayUrl={relayUrl} />
    </div>
  );
}
