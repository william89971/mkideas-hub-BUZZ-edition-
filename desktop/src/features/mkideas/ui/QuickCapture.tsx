import * as React from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Plus } from "lucide-react";

import { publishOrQueueMkState } from "../offlineStore";
import {
  MK_RECORD_KIND_BY_TYPE,
  type MkRecordStatus,
  type MkRecordType,
} from "../model";
import { Button } from "@/shared/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/shared/ui/dialog";
import { Input } from "@/shared/ui/input";
import type { MkIdeasArea } from "./MkIdeasPrimitives";

const CAPTURE_TYPES = [
  "person",
  "task",
  "meeting",
  "content",
  "knowledge",
] as const;
type QuickCaptureType = (typeof CAPTURE_TYPES)[number];

const LABELS: Record<QuickCaptureType, string> = {
  person: "Guest",
  task: "Task",
  meeting: "Meeting",
  content: "Content idea",
  knowledge: "Knowledge note",
};

const INITIAL_STATUS: Record<QuickCaptureType, MkRecordStatus> = {
  person: "prospect",
  task: "to-do",
  meeting: "planned",
  content: "idea",
  knowledge: "draft",
};

const DEFAULT_FOR_AREA: Record<MkIdeasArea, QuickCaptureType> = {
  today: "task",
  work: "task",
  people: "person",
  studio: "content",
  team: "knowledge",
};

export function QuickCapture({
  area,
  relayUrl,
  queryKey,
}: {
  area: MkIdeasArea;
  relayUrl: string;
  queryKey: readonly unknown[];
}) {
  const queryClient = useQueryClient();
  const [open, setOpen] = React.useState(false);
  const [captureType, setCaptureType] = React.useState<QuickCaptureType>(
    DEFAULT_FOR_AREA[area],
  );
  const [title, setTitle] = React.useState("");
  const [detail, setDetail] = React.useState("");
  const [date, setDate] = React.useState("");
  const capture = useMutation({
    mutationFn: () => {
      const fields: Record<string, unknown> =
        captureType === "person"
          ? {
              name: title.trim(),
              organization: detail.trim(),
              do_not_contact: false,
            }
          : captureType === "knowledge"
            ? { title: title.trim(), body: detail.trim() }
            : {
                title: title.trim(),
                ...(detail.trim() ? { notes: detail.trim() } : {}),
                ...(date
                  ? {
                      [captureType === "meeting" ? "scheduled_at" : "due_at"]:
                        new Date(date).toISOString(),
                    }
                  : {}),
              };
      return publishOrQueueMkState(relayUrl, {
        kind: MK_RECORD_KIND_BY_TYPE[captureType],
        recordType: captureType as MkRecordType,
        status: INITIAL_STATUS[captureType],
        fields,
      });
    },
    onSuccess: async () => {
      setTitle("");
      setDetail("");
      setDate("");
      setOpen(false);
      await queryClient.invalidateQueries({ queryKey });
    },
  });
  return (
    <Dialog
      onOpenChange={(nextOpen) => {
        setOpen(nextOpen);
        if (nextOpen) setCaptureType(DEFAULT_FOR_AREA[area]);
      }}
      open={open}
    >
      <DialogTrigger asChild>
        <Button data-testid="mkideas-quick-capture" size="sm">
          <Plus className="h-4 w-4" /> Quick capture
        </Button>
      </DialogTrigger>
      <DialogContent className="max-w-xl rounded-none border border-border bg-[#fbfaf7] dark:bg-background">
        <DialogHeader>
          <DialogTitle className="font-serif text-2xl">
            Quick capture
          </DialogTitle>
          <DialogDescription>
            Put the thought into shared state now. Shape the details with the
            team later.
          </DialogDescription>
        </DialogHeader>
        <form
          className="grid gap-5"
          id="mkideas-quick-capture-form"
          onSubmit={(event) => {
            event.preventDefault();
            capture.mutate();
          }}
        >
          <fieldset className="flex flex-wrap gap-2">
            <legend className="sr-only">Capture type</legend>
            {CAPTURE_TYPES.map((type) => (
              <button
                aria-pressed={captureType === type}
                className={`border px-3 py-2 text-xs font-medium ${
                  captureType === type
                    ? "border-[#b70f22] bg-[#b70f22] text-white"
                    : "border-border bg-background text-muted-foreground hover:text-foreground"
                }`}
                key={type}
                onClick={() => setCaptureType(type)}
                type="button"
              >
                {LABELS[type]}
              </button>
            ))}
          </fieldset>
          <label
            className="grid gap-2 text-xs font-medium"
            htmlFor="mkideas-quick-capture-title"
          >
            {captureType === "person" ? "Guest name" : "Title"}
            <Input
              autoFocus
              id="mkideas-quick-capture-title"
              onChange={(event) => setTitle(event.target.value)}
              placeholder={
                captureType === "person"
                  ? "Who should MK Ideas know?"
                  : `Name this ${LABELS[captureType].toLowerCase()}`
              }
              required
              value={title}
            />
          </label>
          <label
            className="grid gap-2 text-xs font-medium"
            htmlFor="mkideas-quick-capture-detail"
          >
            {captureType === "person" ? "Organization" : "Notes"}
            <textarea
              className="min-h-24 resize-y border border-input bg-background px-3 py-2 text-sm outline-hidden focus-visible:ring-1 focus-visible:ring-ring"
              id="mkideas-quick-capture-detail"
              onChange={(event) => setDetail(event.target.value)}
              placeholder="Optional context"
              value={detail}
            />
          </label>
          {captureType === "task" || captureType === "meeting" ? (
            <label
              className="grid gap-2 text-xs font-medium"
              htmlFor="mkideas-quick-capture-date"
            >
              {captureType === "meeting" ? "Starts" : "Due"}
              <Input
                id="mkideas-quick-capture-date"
                onChange={(event) => setDate(event.target.value)}
                type="datetime-local"
                value={date}
              />
            </label>
          ) : null}
          {capture.error ? (
            <p className="text-xs text-destructive">{capture.error.message}</p>
          ) : null}
        </form>
        <DialogFooter>
          <Button onClick={() => setOpen(false)} type="button" variant="ghost">
            Cancel
          </Button>
          <Button
            disabled={!title.trim() || capture.isPending}
            form="mkideas-quick-capture-form"
            type="submit"
          >
            Save human-signed {LABELS[captureType].toLowerCase()}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
