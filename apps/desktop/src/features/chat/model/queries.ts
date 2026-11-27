import { useState } from "react";
import {
  queryOptions,
  type QueryClient,
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
  type ChatListener,
  type ChatScreenContext,
  type ChatTurnStatus,
  type IpcFailure,
} from "../../../shared/ipc";
import {
  emptyFreeText,
  err,
  ok,
  queryKeys,
  type EntityId,
  type ExternalUrl,
  type FreeText,
  type Result,
} from "../../../shared/lib";
import { detach } from "../../../shared/platform";
import { fromHistory, resumableTurn, type Transcript } from "./transcript";

export type ConversationState = {
  readonly session: EntityId;
  readonly loadId: EntityId;
  readonly transcript: Transcript;
  readonly draft: FreeText;
  readonly resume: EntityId | undefined;
};

type HistoryReply = Result<ConversationState, IpcFailure>;

export type HistoryView =
  | { readonly status: "loading" }
  | { readonly status: "error"; readonly failure: IpcFailure }
  | { readonly status: "ready"; readonly conversation: ConversationState };

const loadHistory = async (
  selected: EntityId | undefined,
): Promise<HistoryReply> => {
  const session = selected === undefined ? mainChatSession() : ok(selected);
  if (session.kind === "err") return session;
  const result = await chatHistory(session.value);
  if (result.kind === "err") return result;
  const loadId = newChatTurn();
  if (loadId.kind === "err") return loadId;
  return ok({
    session: session.value,
    loadId: loadId.value,
    transcript: fromHistory(result.value),
    resume: resumableTurn(result.value),
    draft: emptyFreeText,
  });
};

const historyKey = (
  session: EntityId | undefined,
): readonly [
  ...typeof queryKeys.chatHistory,
  "session",
  EntityId | undefined,
] => [...queryKeys.chatHistory, "session", session] as const;

export const sessionContent = (
  client: QueryClient,
  session: EntityId,
): "empty" | "started" => {
  const reply = client.getQueryData<HistoryReply>(historyKey(session));
  if (reply?.kind !== "ok") return "empty";
  return reply.value.draft === emptyFreeText &&
    reply.value.transcript.exchanges.length === 0
    ? "empty"
    : "started";
};

export const useChatHistory = (session: EntityId | undefined): HistoryView => {
  const query = useQuery(
    queryOptions({
      queryKey: historyKey(session),
      queryFn: () => loadHistory(session),
      staleTime: (query) => (query.state.data?.kind === "ok" ? "static" : 0),
      gcTime: Infinity,
    }),
  );
  if (query.data === undefined) return { status: "loading" };
  if (query.data.kind === "err")
    return { status: "error", failure: query.data.error };
  return { status: "ready", conversation: query.data.value };
};

export const useReloadChatHistory = (): (() => void) => {
  const client = useQueryClient();
  return (): void => {
    detach({
      kind: "invalidate-queries",
      client,
      keys: [queryKeys.chatHistory],
    });
  };
};

type ConversationActions = {
  readonly draft: FreeText;
  readonly setDraft: (draft: FreeText) => void;
  readonly update: (change: (current: Transcript) => Transcript) => void;
};

export const useConversation = (
  conversation: ConversationState,
): ConversationActions => {
  const client = useQueryClient();
  const [draft, setDraft] = useState(conversation.draft);
  const changeState = (
    change: (current: ConversationState) => ConversationState,
  ): void => {
    const query = client.getQueryCache().find<HistoryReply>({
      queryKey: historyKey(conversation.session),
    });
    if (query === undefined) return;
    const reply = query.state.data;
    if (reply?.kind === "ok" && reply.value.loadId === conversation.loadId)
      query.setState({ data: ok(change(reply.value)) });
  };
  return {
    draft,
    setDraft: (next): void => {
      setDraft(next);
      changeState((current) => ({ ...current, draft: next }));
    },
    update: (change): void => {
      changeState((current) => ({
        ...current,
        transcript: change(current.transcript),
      }));
    },
  };
};

export const useChatResume = (
  loadId: EntityId,
  turn: EntityId,
  listener: ChatListener,
): "resuming" | "resumed" => {
  const query = useQuery(
    queryOptions({
      queryKey: [...queryKeys.chatResume, loadId, turn] as const,
      queryFn: () => resubscribeChatTurn(turn, listener),
      staleTime: "static",
      gcTime: Infinity,
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
  session: EntityId,
  hooks: TurnHooks,
  context: ChatScreenContext | undefined,
): ((message: FreeText) => void) => {
  const client = useQueryClient();
  const mutation = useMutation({
    mutationFn: (input: SendInput) => {
      const handle = startChatTurn(
        {
          session,
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
      detach({
        kind: "invalidate-queries",
        client,
        keys: [queryKeys.chatSessions],
      });
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
