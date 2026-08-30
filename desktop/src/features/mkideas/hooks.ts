import * as React from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";

import { useCommunities } from "@/features/communities/useCommunities";
import { fetchMkIdeasSnapshot, subscribeMkIdeas } from "./api";

export function useMkIdeasSnapshot() {
  const communities = useCommunities();
  const relayUrl = communities.activeCommunity?.relayUrl;
  const queryClient = useQueryClient();
  const queryKey = React.useMemo(
    () => ["mkideas", relayUrl ?? "none"] as const,
    [relayUrl],
  );
  const query = useQuery({
    queryKey,
    queryFn: () => fetchMkIdeasSnapshot(relayUrl ?? ""),
    enabled: Boolean(relayUrl),
  });

  React.useEffect(() => {
    if (!relayUrl) return;
    let unsubscribe: (() => Promise<void>) | undefined;
    let cancelled = false;
    void subscribeMkIdeas(relayUrl, () => {
      void queryClient.invalidateQueries({ queryKey });
    }).then((stop) => {
      if (cancelled) void stop();
      else unsubscribe = stop;
    });
    return () => {
      cancelled = true;
      if (unsubscribe) void unsubscribe();
    };
  }, [queryClient, queryKey, relayUrl]);

  return { query, queryKey, relayUrl };
}
