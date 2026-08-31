import * as React from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Plus, Search } from "lucide-react";

import { publishMkState } from "../api";
import type { MkActivity, MkAgentProposal, MkRecord } from "../model";
import { KIND_MK_PERSON } from "@/shared/constants/kinds";
import { Button } from "@/shared/ui/button";
import { Input } from "@/shared/ui/input";
import { AgentWorkbench } from "./AgentWorkbench";
import { EmptyState, PageHeader, Status } from "./MkIdeasPrimitives";
import { PersonProfile } from "./PersonProfile";

export function PeopleArea({
  records,
  proposals,
  activities,
  relayUrl,
  community,
  queryKey,
}: {
  records: MkRecord[];
  proposals: MkAgentProposal[];
  activities: MkActivity[];
  relayUrl: string;
  community: string;
  queryKey: readonly unknown[];
}) {
  const queryClient = useQueryClient();
  const [adding, setAdding] = React.useState(false);
  const [search, setSearch] = React.useState("");
  const [selectedId, setSelectedId] = React.useState("");
  const [name, setName] = React.useState("");
  const [organization, setOrganization] = React.useState("");
  const [why, setWhy] = React.useState("");
  const mutation = useMutation({
    mutationFn: () =>
      publishMkState(relayUrl, {
        kind: KIND_MK_PERSON,
        recordType: "person",
        status: "prospect",
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
      `${record.title} ${String(record.data.organization ?? "")}`
        .toLowerCase()
        .includes(search.toLowerCase()),
  );
  const selected =
    people.find((person) => person.entityId === selectedId) ?? people[0];
  return (
    <div className="space-y-8">
      <PageHeader
        action={
          <Button onClick={() => setAdding((value) => !value)}>
            <Plus className="h-4 w-4" /> Add guest
          </Button>
        }
        area="people"
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

      <section className="grid gap-6 xl:grid-cols-[18rem_1fr]">
        <div>
          <div className="mb-3 flex items-center gap-2 border-b border-border pb-2">
            <Search className="h-4 w-4 text-muted-foreground" />
            <input
              className="w-full bg-transparent text-sm outline-none"
              onChange={(event) => setSearch(event.target.value)}
              placeholder="Filter people"
              value={search}
            />
          </div>
          <div className="border-y border-border">
            {people.map((person) => (
              <button
                className={`w-full border-t border-border px-3 py-4 text-left first:border-t-0 ${selected?.entityId === person.entityId ? "bg-card" : "hover:bg-card/50"}`}
                key={person.entityId}
                onClick={() => setSelectedId(person.entityId)}
                type="button"
              >
                <div className="flex items-center justify-between gap-2">
                  <p className="truncate font-serif text-lg font-semibold">
                    {person.title}
                  </p>
                  <Status>{person.status.replaceAll("_", " ")}</Status>
                </div>
                <p className="mt-1 truncate text-xs text-muted-foreground">
                  {String(person.data.organization ?? "Independent")}
                </p>
              </button>
            ))}
            {!people.length ? (
              <EmptyState>No guests match this view.</EmptyState>
            ) : null}
          </div>
        </div>
        {selected ? (
          <PersonProfile
            activities={activities}
            community={community}
            person={selected}
            proposals={proposals}
            queryKey={queryKey}
            records={records}
            relayUrl={relayUrl}
          />
        ) : (
          <EmptyState>
            Add the first guest to begin a relationship timeline.
          </EmptyState>
        )}
      </section>

      <AgentWorkbench
        activities={activities}
        personaIds={["guest-researcher", "outreach-drafter"]}
        proposals={proposals}
      />
    </div>
  );
}
