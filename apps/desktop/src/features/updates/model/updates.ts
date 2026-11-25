import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type {
  IpcFailure,
  UpdateClient,
  UpdateStatus,
} from "../../../shared/ipc";
import type { Result } from "../../../shared/lib";

const statusKey = ["update-status"] as const;
const busyIntervalMs = 2_000;
const idleIntervalMs = 600_000;

type StatusReply = Result<UpdateStatus, IpcFailure> | undefined;

export type UpdateView =
  | { readonly status: "loading" }
  | { readonly status: "unavailable" }
  | { readonly status: "ready"; readonly data: UpdateStatus };

export const viewOf = (reply: StatusReply): UpdateView => {
  if (reply === undefined) return { status: "loading" };
  if (reply.kind === "err") return { status: "unavailable" };
  return { status: "ready", data: reply.value };
};

type Pace = "busy" | "settled";

const paces = {
  off: "settled",
  idle: "settled",
  checking: "busy",
  downloading: "busy",
  "up-to-date": "settled",
  ready: "settled",
  failed: "settled",
} as const satisfies Record<UpdateStatus["state"]["kind"], Pace>;

export const paceOf = (reply: StatusReply): Pace =>
  reply?.kind === "ok" ? paces[reply.value.state.kind] : "settled";

export const useUpdateStatus = (client: UpdateClient): UpdateView => {
  const query = useQuery({
    queryKey: statusKey,
    queryFn: () => client.status("current"),
    refetchInterval: (current) =>
      paceOf(current.state.data) === "busy" ? busyIntervalMs : idleIntervalMs,
    refetchIntervalInBackground: true,
  });
  return viewOf(query.data);
};

export type UpdateActions = {
  readonly busy: "enabled" | "disabled";
  readonly check: () => void;
  readonly restart: () => void;
};

export const useUpdateActions = (client: UpdateClient): UpdateActions => {
  const queryClient = useQueryClient();
  const remember = (reply: Result<UpdateStatus, IpcFailure>): StatusReply =>
    queryClient.setQueryData<Result<UpdateStatus, IpcFailure>>(
      statusKey,
      reply,
    );
  const check = useMutation({
    mutationFn: () => client.status("check-now"),
    onSuccess: remember,
  });
  const restart = useMutation({
    mutationFn: client.restart,
    onSuccess: remember,
  });
  return {
    busy: check.isPending || restart.isPending ? "disabled" : "enabled",
    check: (): void => {
      check.mutate();
    },
    restart: (): void => {
      restart.mutate();
    },
  };
};
