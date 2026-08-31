import { ArrowRight } from "lucide-react";

import { EmptyState, PageHeader } from "./MkIdeasPrimitives";

export function TeamArea() {
  return (
    <div className="space-y-7">
      <PageHeader area="team" />
      <EmptyState>
        <div className="mx-auto max-w-lg">
          <ArrowRight className="mx-auto mb-3 h-6 w-6" />
          <p className="font-medium text-foreground">
            Choose a channel or direct message
          </p>
          <p className="mt-2">
            Team remains Buzz-native. Copy a record reference from Today, Work,
            People, or Studio into the conversation where the decision happens.
          </p>
        </div>
      </EmptyState>
    </div>
  );
}
