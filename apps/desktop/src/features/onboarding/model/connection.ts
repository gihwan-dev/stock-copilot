import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import type {
  ConnectionState,
  IpcFailure,
  PublicIp,
  SettingsClient,
  TossConnection,
  TossKeys,
} from "../../../shared/ipc";
import type { Result } from "../../../shared/lib";

const connectionKey = ["toss-connection"] as const;
const publicIpKey = ["public-ip"] as const;
const retryIntervalMs = 60_000;

type ConnectionReply = Result<TossConnection, IpcFailure> | undefined;

export type KeyProblem =
  "none" | "invalid-key" | "forbidden" | "account-not-found" | "key-in-use";

export type GateView =
  | { readonly screen: "loading" }
  | { readonly screen: "keys"; readonly problem: KeyProblem }
  | { readonly screen: "ip" }
  | { readonly screen: "ready" };

const viewOfState = (state: ConnectionState): GateView => {
  switch (state) {
    case "connected":
      return { screen: "ready" };
    case "ip-not-allowed":
      return { screen: "ip" };
    case "needs-keys":
      return { screen: "keys", problem: "none" };
    case "invalid-key":
    case "forbidden":
    case "account-not-found":
    case "key-in-use":
      return { screen: "keys", problem: state };
  }
};

export const gateView = (data: ConnectionReply): GateView => {
  if (data === undefined) return { screen: "loading" };
  if (data.kind === "err") return { screen: "ready" };
  return viewOfState(data.value.state);
};

export const useConnectionGate = (client: SettingsClient): GateView => {
  const queryClient = useQueryClient();
  const cached: ConnectionReply = queryClient.getQueryData(connectionKey);
  const query = useQuery({
    queryKey: connectionKey,
    queryFn: client.checkTossConnection,
    staleTime: Infinity,
    refetchInterval: gateView(cached).screen === "ip" ? retryIntervalMs : false,
    refetchIntervalInBackground: true,
  });
  return gateView(query.data);
};

export type IpView =
  | { readonly status: "loading" }
  | { readonly status: "unavailable" }
  | { readonly status: "ready"; readonly ip: PublicIp };

export const usePublicIp = (client: SettingsClient): IpView => {
  const query = useQuery({
    queryKey: publicIpKey,
    queryFn: client.publicIp,
    staleTime: Infinity,
    gcTime: 0,
  });
  if (query.data === undefined) return { status: "loading" };
  if (query.data.kind === "err") return { status: "unavailable" };
  return { status: "ready", ip: query.data.value };
};

type SubmitState = "idle" | "pending" | "failed";

export type KeySubmission = {
  readonly state: SubmitState;
  readonly submit: (keys: TossKeys) => void;
};

export const useKeySubmission = (client: SettingsClient): KeySubmission => {
  const queryClient = useQueryClient();
  const [outcome, setOutcome] = useState<"idle" | "failed">("idle");
  const mutation = useMutation({
    mutationFn: client.submitTossKeys,
    gcTime: 0,
    onSuccess: (reply): ConnectionReply =>
      reply.kind === "ok"
        ? queryClient.setQueryData<Result<TossConnection, IpcFailure>>(
            connectionKey,
            reply,
          )
        : undefined,
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

export type CopyState = "idle" | "copied" | "failed";

export type IpCopy = {
  readonly state: CopyState;
  readonly copy: (ip: PublicIp) => void;
};

export const useIpCopy = (client: SettingsClient): IpCopy => {
  const mutation = useMutation({ mutationFn: client.copyText, gcTime: 0 });
  const done = mutation.data?.kind === "ok" ? "copied" : ("failed" as const);
  return {
    state: mutation.data === undefined ? "idle" : done,
    copy: (ip): void => {
      mutation.mutate(ip.address);
    },
  };
};
