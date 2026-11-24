import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type {
  IpcFailure,
  Principles,
  SettingsClient,
} from "../../../shared/ipc";
import { settingsKeys, type Result } from "../../../shared/lib";

const principlesKey = settingsKeys.principles;

type PrinciplesReply = Result<Principles, IpcFailure>;

export type PrinciplesView =
  | { readonly status: "loading" }
  | { readonly status: "error" }
  | { readonly status: "ready"; readonly data: Principles };

export const usePrinciples = (client: SettingsClient): PrinciplesView => {
  const query = useQuery({
    queryKey: principlesKey,
    queryFn: client.principles,
  });
  if (query.data === undefined) return { status: "loading" };
  if (query.data.kind === "err") return { status: "error" };
  return { status: "ready", data: query.data.value };
};

export type SaveState = "idle" | "pending" | "saved" | "failed";

export type PrinciplesSaver = {
  readonly state: SaveState;
  readonly save: (principles: Principles) => void;
};

export const usePrinciplesSaver = (client: SettingsClient): PrinciplesSaver => {
  const queryClient = useQueryClient();
  const mutation = useMutation({
    mutationFn: client.setPrinciples,
    onSuccess: (reply): PrinciplesReply | undefined =>
      reply.kind === "ok"
        ? queryClient.setQueryData<PrinciplesReply>(principlesKey, reply)
        : undefined,
  });
  const done = mutation.data?.kind === "ok" ? "saved" : ("failed" as const);
  const settled = mutation.data === undefined ? "idle" : done;
  return {
    state: mutation.isPending ? "pending" : settled,
    save: (principles): void => {
      mutation.mutate(principles);
    },
  };
};
