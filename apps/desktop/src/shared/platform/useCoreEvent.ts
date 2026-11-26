import { useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";
import type { CoreUiEvent } from "../ipc";
import { queryKeys, type AppQueryKey } from "../lib";
import { detach, subscribeCoreEvents } from "./detach";

type EventInvalidationPlan =
  | { readonly scope: "all" }
  | { readonly scope: "keys"; readonly keys: readonly AppQueryKey[] };

const plans = {
  all: { scope: "all" },
  settings: {
    scope: "keys",
    keys: [queryKeys.settings, queryKeys.allocation, queryKeys.selfCheck],
  },
  portfolio: {
    scope: "keys",
    keys: [queryKeys.portfolio, queryKeys.allocation],
  },
  transactions: {
    scope: "keys",
    keys: [queryKeys.transactions, queryKeys.portfolio, queryKeys.allocation],
  },
  status: {
    scope: "keys",
    keys: [queryKeys.selfCheck, queryKeys.status, queryKeys.settings],
  },
  activity: { scope: "keys", keys: [] },
  briefing: { scope: "keys", keys: [queryKeys.briefings] },
  navigation: { scope: "keys", keys: [] },
} as const satisfies Record<CoreUiEvent["category"], EventInvalidationPlan>;

const invalidationForCoreEvent = (event: CoreUiEvent): EventInvalidationPlan =>
  plans[event.category];

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

export const useNavigationEvent = (openDataStatus: () => void): void => {
  useEffect(
    () =>
      subscribeCoreEvents((event): void => {
        if (event.category === "navigation") openDataStatus();
      }),
    [openDataStatus],
  );
};
