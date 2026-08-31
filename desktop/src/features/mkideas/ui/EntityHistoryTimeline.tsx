import * as React from "react";
import { History } from "lucide-react";

import { fetchMkEntityHistoryPage } from "../api";
import type { MkRecord } from "../model";
import { Button } from "@/shared/ui/button";

export function EntityHistoryTimeline({
  record,
  relayUrl,
}: {
  record: MkRecord;
  relayUrl: string;
}) {
  const [open, setOpen] = React.useState(false);
  const [records, setRecords] = React.useState<MkRecord[]>([]);
  const [cursor, setCursor] = React.useState<string | null | undefined>();
  const [state, setState] = React.useState<"idle" | "loading" | "error">(
    "idle",
  );

  const loadPage = React.useCallback(
    async (nextCursor?: string) => {
      setState("loading");
      try {
        const page = await fetchMkEntityHistoryPage(relayUrl, {
          kind: record.kind,
          entityId: record.entityId,
          cursor: nextCursor,
        });
        setRecords((current) => {
          const byEvent = new Map(current.map((item) => [item.eventId, item]));
          for (const item of page.records) byEvent.set(item.eventId, item);
          return [...byEvent.values()].sort((a, b) => b.version - a.version);
        });
        setCursor(page.nextCursor);
        setState("idle");
      } catch {
        setState("error");
      }
    },
    [record.entityId, record.kind, relayUrl],
  );

  return (
    <div className="mt-3 border-t border-border pt-3">
      <Button
        onClick={() => {
          const nextOpen = !open;
          setOpen(nextOpen);
          if (nextOpen && cursor === undefined) void loadPage();
        }}
        size="sm"
        variant="ghost"
      >
        <History className="h-4 w-4" /> {open ? "Hide history" : "View history"}
      </Button>
      {open ? (
        <div className="mt-3 border-l border-border pl-4">
          {records.map((revision) => (
            <div className="relative pb-4 last:pb-0" key={revision.eventId}>
              <span className="absolute -left-[1.17rem] top-1.5 h-2 w-2 rounded-full bg-[#b70f22]" />
              <p className="text-xs font-medium">
                v{revision.version} · {revision.status.replaceAll("_", " ")}
              </p>
              <p className="mt-1 text-2xs text-muted-foreground">
                {new Date(revision.createdAt * 1000).toLocaleString()} · signed
                by {revision.author.slice(0, 8)}…
              </p>
            </div>
          ))}
          {state === "loading" ? (
            <p className="text-xs text-muted-foreground">
              Loading signed revisions…
            </p>
          ) : null}
          {state === "error" ? (
            <p className="text-xs text-destructive">
              History is unavailable until the native MK projection bridge is
              ready.
            </p>
          ) : null}
          {cursor ? (
            <Button
              onClick={() => void loadPage(cursor)}
              size="sm"
              variant="outline"
            >
              Load earlier revisions
            </Button>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}
