import * as React from "react";
import { AlertTriangle, CloudOff, RefreshCw, Trash2 } from "lucide-react";

import type { MkRecord } from "../model";
import {
  discardMkOutboxItem,
  flushMkOutbox,
  reapplyMkOutboxItem,
  type MkOutboxItem,
} from "../offlineStore";
import { Button } from "@/shared/ui/button";

function itemTitle(item: MkOutboxItem): string {
  const fields = item.input.fields;
  const title = fields.title ?? fields.name;
  return typeof title === "string" && title.trim()
    ? title
    : `${item.input.recordType} change`;
}

export function MkIdeasSyncPanel({
  relayUrl,
  records,
  outbox,
  pubkey,
  cachedAt,
  isFetchError,
  onChanged,
}: {
  relayUrl: string;
  records: MkRecord[];
  outbox: MkOutboxItem[];
  pubkey?: string;
  cachedAt: string | null;
  isFetchError: boolean;
  onChanged: () => Promise<void>;
}) {
  const [workingId, setWorkingId] = React.useState<string | null>(null);
  const conflicts = outbox.filter((item) => item.status === "conflict");
  const queued = outbox.filter((item) => item.status === "queued");
  if (!isFetchError && outbox.length === 0) return null;

  const run = async (id: string, action: () => Promise<unknown> | unknown) => {
    setWorkingId(id);
    try {
      await action();
      await onChanged();
    } finally {
      setWorkingId(null);
    }
  };

  return (
    <section
      className="mb-5 border border-amber-700/30 bg-amber-50 p-4 text-amber-950 dark:bg-amber-950/20 dark:text-amber-100"
      data-testid="mkideas-sync-panel"
    >
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="flex gap-3">
          {conflicts.length ? (
            <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" />
          ) : (
            <CloudOff className="mt-0.5 h-4 w-4 shrink-0" />
          )}
          <div>
            <p className="text-sm font-semibold">
              {conflicts.length
                ? `${conflicts.length} change${conflicts.length === 1 ? "" : "s"} need review`
                : `${queued.length} change${queued.length === 1 ? "" : "s"} waiting to sync`}
            </p>
            <p className="mt-1 text-xs opacity-80">
              {isFetchError
                ? `Showing the last saved workspace${cachedAt ? ` from ${new Date(cachedAt).toLocaleString()}` : ""}.`
                : "Your signed changes are stored on this device until the relay accepts them."}
            </p>
          </div>
        </div>
        {queued.length ? (
          <Button
            disabled={workingId !== null}
            onClick={() =>
              run("all", () => flushMkOutbox(relayUrl, undefined, pubkey))
            }
            size="sm"
            variant="outline"
          >
            <RefreshCw className="h-4 w-4" /> Retry sync
          </Button>
        ) : null}
      </div>
      {conflicts.length ? (
        <div className="mt-4 grid gap-2">
          {conflicts.map((item) => (
            <div
              className="flex flex-wrap items-center justify-between gap-3 border-t border-amber-800/20 pt-3"
              key={item.id}
            >
              <div>
                <p className="text-sm font-medium">{itemTitle(item)}</p>
                <p className="text-xs opacity-75">
                  Someone changed this record first. Reapply creates a new
                  version from the latest shared copy.
                </p>
              </div>
              <div className="flex gap-2">
                <Button
                  disabled={workingId !== null}
                  onClick={() =>
                    run(item.id, () =>
                      reapplyMkOutboxItem(relayUrl, item, records),
                    )
                  }
                  size="sm"
                  variant="outline"
                >
                  <RefreshCw className="h-4 w-4" /> Reapply
                </Button>
                <Button
                  disabled={workingId !== null}
                  onClick={() => {
                    if (
                      window.confirm(
                        "Discard this saved local change? This cannot be undone.",
                      )
                    ) {
                      void run(item.id, () =>
                        discardMkOutboxItem(relayUrl, item.id),
                      );
                    }
                  }}
                  size="sm"
                  variant="ghost"
                >
                  <Trash2 className="h-4 w-4" /> Discard
                </Button>
              </div>
            </div>
          ))}
        </div>
      ) : null}
    </section>
  );
}
