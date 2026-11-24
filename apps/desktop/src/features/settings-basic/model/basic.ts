import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import type {
  ConnectionState,
  InvestmentProfile,
  IpcFailure,
  OrderSwitch,
  OrderSwitchState,
  SettingsClient,
  TossConnection,
  TossKeys,
} from "../../../shared/ipc";
import type { Result } from "../../../shared/lib";

const connectionKey = ["toss-connection"] as const;
const profileKey = ["investment-profile"] as const;
const orderSwitchKey = ["order-switch"] as const;

export type Loaded<T> =
  | { readonly status: "loading" }
  | { readonly status: "error" }
  | { readonly status: "ready"; readonly data: T };

const loaded = <T>(
  data: Result<T, IpcFailure> | undefined,
  failed: "failed" | "pending",
): Loaded<T> => {
  if (data === undefined)
    return failed === "failed" ? { status: "error" } : { status: "loading" };
  if (data.kind === "err") return { status: "error" };
  return { status: "ready", data: data.value };
};

export type TossStatus = ConnectionState | "unknown" | "loading";

export const useTossStatus = (client: SettingsClient): TossStatus => {
  const query = useQuery({
    queryKey: connectionKey,
    queryFn: client.checkTossConnection,
    staleTime: Infinity,
  });
  const view = loaded(query.data, query.isError ? "failed" : "pending");
  switch (view.status) {
    case "loading":
      return "loading";
    case "error":
      return "unknown";
    case "ready":
      return view.data.state;
  }
};

export type KeyState = "idle" | "pending" | "failed";

export type KeyResubmit = {
  readonly state: KeyState;
  readonly submit: (keys: TossKeys) => void;
};

export const useKeyResubmit = (client: SettingsClient): KeyResubmit => {
  const queryClient = useQueryClient();
  const [outcome, setOutcome] = useState<"idle" | "failed">("idle");
  const remember = (
    reply: Result<TossConnection, IpcFailure> | undefined,
  ): Result<TossConnection, IpcFailure> | undefined =>
    reply?.kind === "ok"
      ? queryClient.setQueryData<Result<TossConnection, IpcFailure>>(
          connectionKey,
          reply,
        )
      : undefined;
  const mutation = useMutation({
    mutationFn: client.submitTossKeys,
    gcTime: 0,
    onSuccess: remember,
  });
  return {
    state: mutation.isPending ? "pending" : outcome,
    submit: (keys): void => {
      setOutcome("idle");
      mutation.mutate(keys, {
        onSettled: (reply): void => {
          setOutcome(reply?.kind === "ok" ? "idle" : "failed");
          mutation.reset();
        },
      });
    },
  };
};

export const useProfile = (
  client: SettingsClient,
): Loaded<InvestmentProfile> => {
  const query = useQuery({
    queryKey: profileKey,
    queryFn: client.investmentProfile,
  });
  return loaded(query.data, query.isError ? "failed" : "pending");
};

export type SaveState = "idle" | "pending" | "saved" | "failed";

export type ProfileSaver = {
  readonly state: SaveState;
  readonly save: (profile: InvestmentProfile) => void;
};

export const useProfileSaver = (client: SettingsClient): ProfileSaver => {
  const queryClient = useQueryClient();
  const mutation = useMutation({
    mutationFn: client.setInvestmentProfile,
    onSuccess: (reply): Result<InvestmentProfile, IpcFailure> | undefined =>
      reply.kind === "ok"
        ? queryClient.setQueryData<Result<InvestmentProfile, IpcFailure>>(
            profileKey,
            reply,
          )
        : undefined,
  });
  const done = mutation.data?.kind === "ok" ? "saved" : ("failed" as const);
  const settled =
    mutation.data === undefined && !mutation.isError ? "idle" : done;
  return {
    state: mutation.isPending ? "pending" : settled,
    save: (profile): void => {
      mutation.mutate(profile);
    },
  };
};

export type OrderControl = {
  readonly view: Loaded<OrderSwitch>;
  readonly pending: "pending" | "idle";
  readonly failed: "failed" | "idle";
  readonly request: (state: OrderSwitchState) => void;
};

export const useOrderSwitch = (client: SettingsClient): OrderControl => {
  const queryClient = useQueryClient();
  const query = useQuery({
    queryKey: orderSwitchKey,
    queryFn: client.orderSwitch,
  });
  const mutation = useMutation({
    mutationFn: client.requestOrderSwitch,
    onSuccess: (reply): Result<OrderSwitch, IpcFailure> | undefined =>
      reply.kind === "ok"
        ? queryClient.setQueryData<Result<OrderSwitch, IpcFailure>>(
            orderSwitchKey,
            reply,
          )
        : undefined,
  });
  const failed =
    mutation.isError || mutation.data?.kind === "err" ? "failed" : "idle";
  return {
    view: loaded(query.data, query.isError ? "failed" : "pending"),
    pending: mutation.isPending ? "pending" : "idle",
    failed,
    request: (state): void => {
      mutation.mutate(state);
    },
  };
};
