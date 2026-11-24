import {
  useMutation,
  useQuery,
  useQueryClient,
  type QueryClient,
} from "@tanstack/react-query";
import type {
  AccountRef,
  AccountSelection,
  IpcFailure,
  SettingsClient,
} from "../../../shared/ipc";
import type { Result } from "../../../shared/lib";

const accountsKey = ["account-selection"] as const;

type AccountsReply = Result<AccountSelection, IpcFailure>;

export type AccountsView =
  | { readonly status: "loading" }
  | { readonly status: "error" }
  | { readonly status: "ready"; readonly selection: AccountSelection };

export const useAccounts = (client: SettingsClient): AccountsView => {
  const query = useQuery({
    queryKey: accountsKey,
    queryFn: client.accountSelection,
  });
  if (query.data === undefined) return { status: "loading" };
  if (query.data.kind === "err") return { status: "error" };
  return { status: "ready", selection: query.data.value };
};

const accountChanged = (
  queryClient: QueryClient,
  reply: AccountsReply,
): Promise<AccountsReply | undefined> =>
  queryClient
    .invalidateQueries({
      predicate: (query) => query.queryKey[0] !== accountsKey[0],
    })
    .then(() => queryClient.setQueryData<AccountsReply>(accountsKey, reply));

export type ChangeState = "idle" | "pending" | "failed";

export type AccountChange = {
  readonly state: ChangeState;
  readonly select: (id: AccountRef) => void;
};

export const useAccountChange = (client: SettingsClient): AccountChange => {
  const queryClient = useQueryClient();
  const mutation = useMutation({
    mutationFn: client.selectAccount,
    onSuccess: (reply): Promise<AccountsReply | undefined> | undefined =>
      reply.kind === "ok" ? accountChanged(queryClient, reply) : undefined,
  });
  const failed = mutation.data?.kind === "err" ? "failed" : ("idle" as const);
  return {
    state: mutation.isPending ? "pending" : failed,
    select: (id): void => {
      mutation.mutate(id);
    },
  };
};

export type ActionOutcome =
  | "idle"
  | "pending"
  | "backed-up"
  | "exported"
  | "cancelled"
  | "deleted"
  | "failed";

export type DataActions = {
  readonly outcome: ActionOutcome;
  readonly backup: () => void;
  readonly exportData: () => void;
  readonly deleteAll: () => void;
};

type ActionReply = Result<{ readonly status: ActionOutcome }, IpcFailure>;

const outcomeOf = (reply: ActionReply | undefined): ActionOutcome => {
  if (reply === undefined) return "idle";
  return reply.kind === "ok" ? reply.value.status : "failed";
};

const runAction = (
  client: SettingsClient,
  action: "backup" | "export" | "delete",
): Promise<ActionReply> => {
  switch (action) {
    case "backup":
      return client.backupNow();
    case "export":
      return client.exportData();
    case "delete":
      return client.deleteAllData();
  }
};

export const useDataActions = (client: SettingsClient): DataActions => {
  const queryClient = useQueryClient();
  const mutation = useMutation({
    mutationFn: (action: "backup" | "export" | "delete") =>
      runAction(client, action),
    onSuccess: (reply): Promise<void> | undefined =>
      reply.kind === "ok" && reply.value.status === "deleted"
        ? queryClient.invalidateQueries()
        : undefined,
  });
  return {
    outcome: mutation.isPending ? "pending" : outcomeOf(mutation.data),
    backup: (): void => {
      mutation.mutate("backup");
    },
    exportData: (): void => {
      mutation.mutate("export");
    },
    deleteAll: (): void => {
      mutation.mutate("delete");
    },
  };
};
