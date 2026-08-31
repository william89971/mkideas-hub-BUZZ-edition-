import * as React from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { CalendarDays, Plus } from "lucide-react";

import { publishMkState } from "../api";
import {
  isMkStatusFor,
  MK_RECORD_KIND_BY_TYPE,
  MK_STATUSES,
  type MkRecord,
  type MkRecordStatus,
  type MkRecordType,
} from "../model";
import { Button } from "@/shared/ui/button";
import { Input } from "@/shared/ui/input";
import { EmptyState, PageHeader, RecordRow } from "./MkIdeasPrimitives";

const WORK_TYPES = ["goal", "project", "task", "meeting", "decision"] as const;
type WorkRecordType = (typeof WORK_TYPES)[number];

const INITIAL_STATUS: Record<WorkRecordType, MkRecordStatus> = {
  goal: "draft",
  project: "planned",
  task: "to-do",
  meeting: "planned",
  decision: "proposed",
};

const LABELS: Record<WorkRecordType, string> = {
  goal: "Goals",
  project: "Projects",
  task: "Tasks",
  meeting: "Meetings",
  decision: "Decisions",
};

export function WorkArea({
  records,
  relayUrl,
  community,
  queryKey,
}: {
  records: MkRecord[];
  relayUrl: string;
  community: string;
  queryKey: readonly unknown[];
}) {
  const queryClient = useQueryClient();
  const [activeType, setActiveType] = React.useState<WorkRecordType>("task");
  const [adding, setAdding] = React.useState(false);
  const [createType, setCreateType] = React.useState<WorkRecordType>("task");
  const [title, setTitle] = React.useState("");
  const [dueAt, setDueAt] = React.useState("");
  const workRecords = records.filter((record) =>
    (WORK_TYPES as readonly MkRecordType[]).includes(record.recordType),
  );
  const visible = workRecords.filter(
    (record) => record.recordType === activeType,
  );

  const create = useMutation({
    mutationFn: () =>
      publishMkState(relayUrl, {
        kind: MK_RECORD_KIND_BY_TYPE[createType],
        recordType: createType,
        status: INITIAL_STATUS[createType],
        fields: {
          title: title.trim(),
          ...(dueAt ? { due_at: new Date(dueAt).toISOString() } : {}),
        },
      }),
    onSuccess: async () => {
      setActiveType(createType);
      setTitle("");
      setDueAt("");
      setAdding(false);
      await queryClient.invalidateQueries({ queryKey });
    },
  });
  const changeStatus = useMutation({
    mutationFn: ({ record, status }: { record: MkRecord; status: string }) => {
      if (!isMkStatusFor(record.recordType, status)) {
        throw new Error("That status is not valid for this record.");
      }
      return publishMkState(relayUrl, {
        kind: record.kind,
        recordType: record.recordType,
        previous: record,
        status,
        fields: record.data,
      });
    },
    onSuccess: () => queryClient.invalidateQueries({ queryKey }),
  });

  return (
    <div className="space-y-7">
      <PageHeader
        action={
          <Button onClick={() => setAdding((value) => !value)}>
            <Plus className="h-4 w-4" /> New work item
          </Button>
        }
        area="work"
      />
      {adding ? (
        <form
          className="grid gap-4 border border-border bg-card p-5 md:grid-cols-[10rem_1fr_12rem_auto] md:items-end"
          onSubmit={(event) => {
            event.preventDefault();
            create.mutate();
          }}
        >
          <label
            className="grid gap-2 text-xs font-medium"
            htmlFor="mkideas-work-type"
          >
            Type
            <select
              className="h-9 border border-border bg-background px-3 text-sm"
              id="mkideas-work-type"
              onChange={(event) =>
                setCreateType(event.target.value as WorkRecordType)
              }
              value={createType}
            >
              {WORK_TYPES.map((type) => (
                <option key={type} value={type}>
                  {LABELS[type].slice(0, -1)}
                </option>
              ))}
            </select>
          </label>
          <label
            className="grid gap-2 text-xs font-medium"
            htmlFor="mkideas-work-title"
          >
            Title
            <Input
              autoFocus
              id="mkideas-work-title"
              onChange={(event) => setTitle(event.target.value)}
              placeholder={`Name this ${createType}`}
              required
              value={title}
            />
          </label>
          <label
            className="grid gap-2 text-xs font-medium"
            htmlFor="mkideas-work-due"
          >
            Date or deadline
            <Input
              id="mkideas-work-due"
              onChange={(event) => setDueAt(event.target.value)}
              type="datetime-local"
              value={dueAt}
            />
          </label>
          <div className="flex gap-2">
            <Button disabled={!title.trim() || create.isPending} type="submit">
              Save
            </Button>
            <Button
              onClick={() => setAdding(false)}
              type="button"
              variant="ghost"
            >
              Cancel
            </Button>
          </div>
          {create.error ? (
            <p className="text-xs text-destructive md:col-span-4">
              {create.error.message}
            </p>
          ) : null}
        </form>
      ) : null}
      <nav
        aria-label="Work record types"
        className="flex gap-1 overflow-x-auto border-b border-border"
      >
        {WORK_TYPES.map((type) => {
          const count = workRecords.filter(
            (record) => record.recordType === type,
          ).length;
          return (
            <button
              aria-current={activeType === type ? "page" : undefined}
              className={`border-b-2 px-4 py-3 text-sm font-medium ${
                activeType === type
                  ? "border-[#b70f22] text-foreground"
                  : "border-transparent text-muted-foreground hover:text-foreground"
              }`}
              key={type}
              onClick={() => setActiveType(type)}
              type="button"
            >
              {LABELS[type]} <span className="text-xs">{count}</span>
            </button>
          );
        })}
      </nav>
      <section className="border-y border-border">
        {visible.length ? (
          visible.map((record) => (
            <RecordRow
              community={community}
              key={record.entityId}
              record={record}
              meta={
                <div className="flex flex-wrap items-center justify-end gap-3">
                  {typeof record.data.due_at === "string" ? (
                    <span className="inline-flex items-center gap-1 text-xs text-muted-foreground">
                      <CalendarDays className="h-3.5 w-3.5" />
                      {new Date(record.data.due_at).toLocaleString()}
                    </span>
                  ) : null}
                  <label
                    className="sr-only"
                    htmlFor={`status-${record.entityId}`}
                  >
                    Status for {record.title}
                  </label>
                  <select
                    className="h-8 border border-border bg-background px-2 text-xs"
                    disabled={changeStatus.isPending}
                    id={`status-${record.entityId}`}
                    onChange={(event) =>
                      changeStatus.mutate({
                        record,
                        status: event.target.value,
                      })
                    }
                    value={record.status}
                  >
                    {MK_STATUSES[record.recordType].map((status) => (
                      <option key={status} value={status}>
                        {status.replaceAll("-", " ")}
                      </option>
                    ))}
                  </select>
                </div>
              }
            />
          ))
        ) : (
          <EmptyState>
            No {LABELS[activeType].toLowerCase()} yet. Capture the first one to
            make the operating plan visible to the team.
          </EmptyState>
        )}
      </section>
      {changeStatus.error ? (
        <p className="text-xs text-destructive">{changeStatus.error.message}</p>
      ) : null}
    </div>
  );
}
