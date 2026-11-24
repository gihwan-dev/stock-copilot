import { useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";
import type { CoreUiEvent } from "../ipc";
import { queryKeys, type AppQueryKey } from "../lib";
import { detach, subscribeCoreEvents } from "./detach";

type EventInvalidationPlan =
  | { readonly scope: "all" }
  | { readonly scope: "keys"; readonly keys: readonly AppQueryKey[] };

const invalidationForCoreEvent = (
  event: CoreUiEvent,
): EventInvalidationPlan => {
  switch (event.category) {
    case "all":
      return { scope: "all" };
    case "settings":
      return {
        scope: "keys",
        keys: [queryKeys.settings, queryKeys.allocation, queryKeys.selfCheck],
      };
    case "portfolio":
      return {
        scope: "keys",
        keys: [queryKeys.portfolio, queryKeys.allocation],
      };
    case "transactions":
      return {
        scope: "keys",
        keys: [
          queryKeys.transactions,
          queryKeys.portfolio,
          queryKeys.allocation,
        ],
      };
    case "status":
      return {
        scope: "keys",
        keys: [queryKeys.selfCheck, queryKeys.status, queryKeys.settings],
      };
    case "activity":
      return { scope: "keys", keys: [] };
    case "briefing":
      return { scope: "keys", keys: [queryKeys.briefings] };
  }
};

export const useCoreEvent = (): void => {
  const client = useQueryClient();
  useEffect(
    () =>
      subscribeCoreEvents((event): void => {
        const plan = invalidationForCoreEvent(event);
        if (plan.scope === "all") {
          detach({ kind: "invalidate-all", client });
        } else if (plan.keys.length > 0) {
          detach({ kind: "invalidate-queries", client, keys: plan.keys });
        }
      }),
    [client],
  );
};
