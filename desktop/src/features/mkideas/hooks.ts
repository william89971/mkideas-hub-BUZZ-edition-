import * as React from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";

import { useCommunities } from "@/features/communities/useCommunities";
import { useIdentityQuery } from "@/shared/api/hooks";
import { fetchMkIdeasEvents, subscribeMkIdeas } from "./api";
import { parseMkIdeasEvents } from "./model";
import {
  cacheMkIdeasEvents,
  flushMkOutbox,
  readCachedMkIdeasSnapshot,
  readMkOutbox,
  subscribeMkOutbox,
} from "./offlineStore";

export function useMkIdeasSnapshot() {
  const communities = useCommunities();
  const identityQuery = useIdentityQuery();
  const relayUrl = communities.activeCommunity?.relayUrl;
  const pubkey = identityQuery.data?.pubkey;
  const queryClient = useQueryClient();
  const queryKey = React.useMemo(
    () => ["mkideas", relayUrl ?? "none"] as const,
    [relayUrl],
  );
  const cached = React.useMemo(
    () => (relayUrl ? readCachedMkIdeasSnapshot(relayUrl) : null),
    [relayUrl],
  );
  const query = useQuery({
    queryKey,
    queryFn: async () => {
      const events = await fetchMkIdeasEvents(relayUrl ?? "");
      cacheMkIdeasEvents(relayUrl ?? "", events);
      return parseMkIdeasEvents(events);
    },
    enabled: Boolean(relayUrl),
    initialData: cached?.snapshot,
  });

  React.useEffect(() => {
    if (!relayUrl) return;
    let unsubscribe: (() => Promise<void>) | undefined;
    let cancelled = false;
    void subscribeMkIdeas(relayUrl, () => {
      void queryClient.invalidateQueries({ queryKey });
    }).then((stop) => {
      if (pubkey) {
        void flushMkOutbox(relayUrl, undefined, pubkey).then(() => {
          void queryClient.invalidateQueries({ queryKey });
        });
      }
      if (cancelled) void stop();
      else unsubscribe = stop;
    });
    return () => {
      cancelled = true;
      if (unsubscribe) void unsubscribe();
    };
  }, [pubkey, queryClient, queryKey, relayUrl]);

  React.useEffect(() => {
    if (!relayUrl || !pubkey) return;
    const flush = () => {
      void flushMkOutbox(relayUrl, undefined, pubkey).then(() => {
        void queryClient.invalidateQueries({ queryKey });
      });
    };
    window.addEventListener("online", flush);
    return () => window.removeEventListener("online", flush);
  }, [pubkey, queryClient, queryKey, relayUrl]);

  const [outbox, setOutbox] = React.useState(() =>
    relayUrl && pubkey ? readMkOutbox(relayUrl, undefined, pubkey) : [],
  );
  React.useEffect(() => {
    const refresh = () =>
      setOutbox(
        relayUrl && pubkey ? readMkOutbox(relayUrl, undefined, pubkey) : [],
      );
    refresh();
    return subscribeMkOutbox(refresh);
  }, [pubkey, relayUrl]);

  return {
    query,
    queryKey,
    relayUrl,
    pubkey,
    outbox,
    cachedAt: cached?.savedAt ?? null,
  };
}
