import * as React from "react";
import { Copy } from "lucide-react";

import type { MkRecord } from "../model";
import { buildMkIdeasLink } from "@/shared/lib/entityLink";

export type MkIdeasArea = "today" | "work" | "people" | "studio" | "team";

const AREA_COPY: Record<
  MkIdeasArea,
  { eyebrow: string; title: string; note: string }
> = {
  today: {
    eyebrow: "COMMAND DESK",
    title: "Today",
    note: "The decisions, handoffs, and interviews that need a human eye.",
  },
  work: {
    eyebrow: "OPERATING RHYTHM",
    title: "Work",
    note: "Goals, projects, tasks, meetings, and decisions in one shared view.",
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

export function EmptyState({ children }: { children: React.ReactNode }) {
  return (
    <div className="border border-dashed border-border bg-background/50 p-8 text-center text-sm text-muted-foreground">
      {children}
    </div>
  );
}

export function Status({ children }: { children: React.ReactNode }) {
  return (
    <span className="inline-flex rounded-full border border-border bg-background px-2 py-0.5 text-2xs font-medium uppercase tracking-wide text-muted-foreground">
      {children}
    </span>
  );
}

export function TeamReferenceButton({
  label,
  reference,
}: {
  label: string;
  reference: string;
}) {
  const [state, setState] = React.useState<"idle" | "copied" | "error">("idle");
  return (
    <button
      className="inline-flex items-center gap-1 text-xs font-medium text-muted-foreground hover:text-foreground"
      onClick={async () => {
        try {
          await navigator.clipboard.writeText(`${label} — ${reference}`);
          setState("copied");
        } catch {
          setState("error");
        }
        window.setTimeout(() => setState("idle"), 1600);
      }}
      type="button"
    >
      <Copy className="h-3.5 w-3.5" />
      {state === "copied"
        ? "Copied for Team"
        : state === "error"
          ? "Copy failed"
          : "Copy Team reference"}
    </button>
  );
}

export function RecordRow({
  record,
  community,
  meta,
}: {
  record: MkRecord;
  community: string;
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
              community,
              kind: record.kind,
              id: record.entityId,
            })}
          />
        </div>
      </div>
      {meta}
    </article>
  );
}

export function PageHeader({
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
