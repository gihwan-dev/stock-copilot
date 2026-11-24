import {
  queryOptions,
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import {
  cancelChatTurn,
  chatHistory,
  mainChatSession,
  newChatTurn,
  openExternal,
  resubscribeChatTurn,
  startChatTurn,
  type ChatHistory,
  type ChatListener,
  type ChatScreenContext,
  type ChatTurnStatus,
  type IpcFailure,
} from "../../../shared/ipc";
import {
  err,
  queryKeys,
  type EntityId,
  type ExternalUrl,
  type FreeText,
  type Result,
} from "../../../shared/lib";

export type HistoryView =
  | { readonly status: "loading" }
  | { readonly status: "error"; readonly failure: IpcFailure }
  | { readonly status: "ready"; readonly history: ChatHistory };

const loadHistory = (): Promise<Result<ChatHistory, IpcFailure>> => {
  const session = mainChatSession();
  return session.kind === "err"
    ? Promise.resolve(session)
    : chatHistory(session.value);
};

const historyOptions = queryOptions({
  queryKey: queryKeys.chatHistory,
  queryFn: loadHistory,
});

export const useChatHistory = (): HistoryView => {
  const query = useQuery(historyOptions);
  if (query.data === undefined) return { status: "loading" };
  if (query.data.kind === "err")
    return { status: "error", failure: query.data.error };
  return { status: "ready", history: query.data.value };
};

export const useReloadChatHistory = (): (() => void) => {
  const client = useQueryClient();
  return (): void => {
    client.removeQueries({ queryKey: queryKeys.chatHistory });
  };
};

export const useChatResume = (
  turn: EntityId,
  listener: ChatListener,
): "resuming" | "resumed" => {
  const query = useQuery(
    queryOptions({
      queryKey: [...queryKeys.chatResume, turn] as const,
      queryFn: () => resubscribeChatTurn(turn, listener),
      staleTime: Infinity,
      gcTime: 0,
    }),
  );
  return query.data === undefined ? "resuming" : "resumed";
};

export type TurnHooks = {
  readonly begin: (turn: EntityId, message: FreeText) => void;
  readonly listener: ChatListener;
  readonly settle: (
    turn: EntityId,
    status: ChatTurnStatus | "unreachable",
  ) => void;
};

type SendInput = { readonly turn: EntityId; readonly message: FreeText };

export const useSendChat = (
  hooks: TurnHooks,
  context: ChatScreenContext | undefined,
): ((message: FreeText) => void) => {
  const mutation = useMutation({
    mutationFn: (input: SendInput) => {
      const session = mainChatSession();
      if (session.kind === "err") return Promise.resolve(session);
      const handle = startChatTurn(
        {
          session: session.value,
          turn: input.turn,
          message: input.message,
          context,
        },
        hooks.listener,
      );
      return handle.kind === "err"
        ? Promise.resolve(err(handle.error))
        : handle.value.settled;
    },
    onSuccess: (result, input): void => {
      hooks.settle(
        input.turn,
        result.kind === "ok" ? result.value.status : "unreachable",
      );
    },
  });
  return (message): void => {
    const turn = newChatTurn();
    if (turn.kind === "err") return;
    hooks.begin(turn.value, message);
    mutation.mutate({ turn: turn.value, message });
  };
};

export const useCancelChat = (): ((turn: EntityId) => void) => {
  const mutation = useMutation({ mutationFn: cancelChatTurn });
  return (turn): void => {
    mutation.mutate(turn);
  };
};

export const useOpenLink = (): ((url: ExternalUrl) => void) => {
  const mutation = useMutation({ mutationFn: openExternal });
  return (url): void => {
    mutation.mutate(url);
  };
};
