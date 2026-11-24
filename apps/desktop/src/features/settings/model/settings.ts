import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  getSettings,
  setOrderPermission,
  type IpcFailure,
  type OrderPermissionState,
  type OrderSwitchOutcome,
  type SettingsView,
} from "../../../shared/ipc";
import { queryKeys, type Result } from "../../../shared/lib";
import { detach } from "../../../shared/platform";

export type SettingsQueryState =
  | { readonly status: "loading" }
  | { readonly status: "error"; readonly failure: IpcFailure }
  | { readonly status: "ready"; readonly data: SettingsView };

export type OrderSwitchFeedback =
  "idle" | "submitting" | "changed" | "denied" | "unavailable" | "failed";

const toSettingsState = (
  result: Result<SettingsView, IpcFailure> | undefined,
): SettingsQueryState => {
  if (result === undefined) return { status: "loading" };
  if (result.kind === "err") return { status: "error", failure: result.error };
  return { status: "ready", data: result.value };
};

export const useSettingsQuery = (): {
  readonly view: SettingsQueryState;
  readonly retry: () => void;
} => {
  const queryClient = useQueryClient();
  const query = useQuery({
    queryKey: queryKeys.settings,
    queryFn: getSettings,
  });
  const retry = (): void => {
    detach({
      kind: "invalidate-queries",
      client: queryClient,
      keys: [queryKeys.settings],
    });
  };
  return { view: toSettingsState(query.data), retry };
};

const mapOrderOutcome = (
  result: Result<OrderSwitchOutcome, IpcFailure> | undefined,
  isPending: "pending" | "idle",
): OrderSwitchFeedback => {
  if (isPending === "pending") return "submitting";
  if (result === undefined) return "idle";
  if (result.kind === "err") return "failed";
  switch (result.value.kind) {
    case "changed":
    case "unchanged":
      return "changed";
    case "denied":
      return "denied";
    case "unavailable":
      return "unavailable";
  }
};

export const useOrderPermissionSwitch = (): {
  readonly feedback: OrderSwitchFeedback;
  readonly toggle: (next: OrderPermissionState) => void;
} => {
  const queryClient = useQueryClient();
  const mutation = useMutation({
    mutationFn: setOrderPermission,
    onSuccess: (): void => {
      detach({
        kind: "invalidate-queries",
        client: queryClient,
        keys: [queryKeys.settings],
      });
    },
  });
  const toggle = (next: OrderPermissionState): void => {
    mutation.mutate(next);
  };
  return {
    feedback: mapOrderOutcome(
      mutation.data,
      mutation.isPending ? "pending" : "idle",
    ),
    toggle,
  };
};
