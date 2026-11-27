import {
  useMutation,
  useQuery,
  useQueryClient,
  type QueryClient,
} from "@tanstack/react-query";
import {
  chatSessions,
  createChatSession,
  mainChatSession,
  newChatSessionId,
  type ChatSessionItem,
  type IpcFailure,
} from "../../../shared/ipc";
import { ok, queryKeys, type EntityId, type Result } from "../../../shared/lib";
import { detach } from "../../../shared/platform";
import { sessionContent } from "./queries";

type Items = readonly ChatSessionItem[];
type Listing = {
  readonly state: "ready" | "failed";
  readonly stored: Items | undefined;
};

export type Directory = {
  readonly entries: readonly ChatSessionItem[];
  readonly selected: EntityId | undefined;
  readonly main: EntityId | undefined;
  readonly state: "loading" | "failed" | "ready";
  readonly creation: "pending" | "failed" | "ready";
  readonly select: (session: EntityId) => void;
  readonly create: () => void;
  readonly retry: () => void;
};

const loadList = async (client: QueryClient): Promise<Listing> => {
  const result = await chatSessions();
  if (result.kind === "ok") return { state: "ready", stored: result.value };
  const previous = client.getQueryData<Listing>(queryKeys.chatSessions);
  return { state: "failed", stored: previous?.stored };
};

const entriesFor = (
  stored: Items | undefined,
  main: EntityId | undefined,
): Items => {
  if (stored?.length === 0 && main !== undefined)
    return [{ session: main, title: undefined }];
  return stored ?? [];
};

const preserveFirstDraft = async (
  client: QueryClient,
  stored: Items,
  main: EntityId | undefined,
): Promise<Result<ChatSessionItem, IpcFailure> | undefined> => {
  if (
    stored.length === 0 &&
    main !== undefined &&
    sessionContent(client, main) === "started"
  )
    return createChatSession(main);
  return undefined;
};

const create = async (
  client: QueryClient,
  stored: Items,
  main: EntityId | undefined,
): Promise<Result<Items, IpcFailure>> => {
  const id = newChatSessionId();
  if (id.kind === "err") return id;
  const previous = await preserveFirstDraft(client, stored, main);
  if (previous?.kind === "err") return previous;
  const next = await createChatSession(id.value);
  if (next.kind === "err") return next;
  return ok(
    previous === undefined ? [next.value] : [next.value, previous.value],
  );
};

const useSelection = (
  first: EntityId | undefined,
): readonly [EntityId | null | undefined, Directory["select"]] => {
  const client = useQueryClient();
  const selection = useQuery<EntityId | null>({
    queryKey: queryKeys.chatSelection,
    queryFn: () => first ?? null,
    enabled: first !== undefined,
    staleTime: "static",
    gcTime: Infinity,
  });
  const select = (session: EntityId): void => {
    const query = client
      .getQueryCache()
      .find<EntityId | null>({ queryKey: queryKeys.chatSelection });
    if (query !== undefined) query.setState({ data: session });
  };
  return [selection.data, select];
};

const useCreateSession = (
  main: EntityId | undefined,
  stored: Items | undefined,
  select: Directory["select"],
): Pick<Directory, "creation" | "create"> => {
  const client = useQueryClient();
  const mutation = useMutation({
    mutationFn: () => create(client, stored ?? [], main),
    onSuccess: async (result): Promise<void> => {
      if (result.kind === "err") return;
      await client.cancelQueries({ queryKey: queryKeys.chatSessions });
      const query = client
        .getQueryCache()
        .find<Listing>({ queryKey: queryKeys.chatSessions });
      const other = query?.state.data?.stored ?? [];
      if (query !== undefined)
        query.setState({
          data: {
            state: "ready",
            stored: [
              ...result.value,
              ...other.filter(
                (entry) =>
                  !result.value.some(
                    (added) => added.session === entry.session,
                  ),
              ),
            ],
          },
        });
      const next = result.value[0];
      if (next !== undefined) select(next.session);
    },
  });
  const completed = mutation.data?.kind === "err" ? "failed" : "ready";
  return {
    creation: mutation.isPending ? "pending" : completed,
    create: (): void => {
      mutation.mutate();
    },
  };
};

export const useSessionDirectory = (): Directory => {
  const client = useQueryClient();
  const mainId = mainChatSession();
  const main = mainId.kind === "ok" ? mainId.value : undefined;
  const list = useQuery({
    queryKey: queryKeys.chatSessions,
    queryFn: () => loadList(client),
  });
  const stored = list.data?.stored;
  const entries = entriesFor(stored, main);
  const first = entries[0]?.session;
  const [selected, select] = useSelection(first);
  const creation = useCreateSession(main, stored, select);
  return {
    entries,
    main,
    ...creation,
    select,
    selected:
      entries.find((entry) => entry.session === selected)?.session ?? first,
    state: list.data?.state ?? "loading",
    retry: (): void => {
      detach({
        kind: "invalidate-queries",
        client,
        keys: [queryKeys.chatSessions],
      });
    },
  };
};
