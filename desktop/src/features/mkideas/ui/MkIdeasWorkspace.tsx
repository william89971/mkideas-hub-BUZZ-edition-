import { useMkIdeasSnapshot } from "../hooks";
import { communityHost } from "../model";
import { EmptyState, type MkIdeasArea } from "./MkIdeasPrimitives";
import { PeopleArea } from "./PeopleArea";
import { QuickCapture } from "./QuickCapture";
import { StudioArea } from "./StudioArea";
import { TeamArea } from "./TeamArea";
import { TodayArea } from "./TodayArea";
import { WorkArea } from "./WorkArea";

export type { MkIdeasArea } from "./MkIdeasPrimitives";

export function MkIdeasWorkspace({ area }: { area: MkIdeasArea }) {
  const { query, queryKey, relayUrl } = useMkIdeasSnapshot();
  const snapshot = query.data ?? {
    records: [],
    proposals: [],
    approvalActions: [],
    activities: [],
    summaries: [],
  };
  if (!relayUrl) {
    return (
      <EmptyState>
        Connect to the private MK Ideas community to continue.
      </EmptyState>
    );
  }
  const community = communityHost(relayUrl);
  return (
    <main
      className="h-full overflow-y-auto bg-[#fbfaf7] px-6 py-8 text-[#141415] dark:bg-background dark:text-foreground lg:px-10"
      data-testid={`mkideas-${area}`}
      key={area}
    >
      <div className="mx-auto max-w-6xl">
        <div className="mb-5 flex justify-end">
          <QuickCapture area={area} queryKey={queryKey} relayUrl={relayUrl} />
        </div>
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
            community={community}
            activities={snapshot.activities}
            approvalActions={snapshot.approvalActions}
            proposals={snapshot.proposals}
            queryKey={queryKey}
            records={snapshot.records}
            relayUrl={relayUrl}
            summaries={snapshot.summaries}
          />
        ) : null}
        {area === "work" ? (
          <WorkArea
            community={community}
            queryKey={queryKey}
            records={snapshot.records}
            relayUrl={relayUrl}
          />
        ) : null}
        {area === "people" ? (
          <PeopleArea
            community={community}
            activities={snapshot.activities}
            proposals={snapshot.proposals}
            queryKey={queryKey}
            records={snapshot.records}
            relayUrl={relayUrl}
          />
        ) : null}
        {area === "studio" ? (
          <StudioArea
            activities={snapshot.activities}
            community={community}
            approvalActions={snapshot.approvalActions}
            proposals={snapshot.proposals}
            queryKey={queryKey}
            records={snapshot.records}
            relayUrl={relayUrl}
          />
        ) : null}
        {area === "team" ? <TeamArea /> : null}
      </div>
    </main>
  );
}
