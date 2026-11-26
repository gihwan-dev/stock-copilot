import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  openExternal,
  toScreenState,
  type InstrumentDetailView,
  type InstrumentListView,
  type ScreenSources,
  type ScreenState,
  type ThesisView,
  type WatchlistChange,
} from "../../../shared/ipc";
import {
  parseTicker,
  queryKeys,
  screenKeys,
  type EntityId,
  type ExternalUrl,
  type FreeText,
  type InputValue,
} from "../../../shared/lib";
import { detach } from "../../../shared/platform";

export const useOpenArticle = (): ((url: ExternalUrl) => void) => {
  const mutation = useMutation({ mutationFn: openExternal });
  return (url): void => {
    mutation.mutate(url);
  };
};

const useRefresh = (): (() => void) => {
  const client = useQueryClient();
  return (): void => {
    detach({
      kind: "invalidate-queries",
      client,
      keys: [queryKeys.instruments],
    });
  };
};

export const useInstrumentList = (
  load: ScreenSources["instruments"],
): {
  readonly view: ScreenState<InstrumentListView>;
  readonly retry: () => void;
} => {
  const query = useQuery({ queryKey: screenKeys.instruments, queryFn: load });
  return {
    view: toScreenState(query.data, (data) =>
      data.rows.length === 0 ? "empty" : "filled",
    ),
    retry: useRefresh(),
  };
};

export const useInstrumentDetail = (
  load: ScreenSources["instrumentDetail"],
  id: EntityId,
): {
  readonly view: ScreenState<InstrumentDetailView>;
  readonly retry: () => void;
} => {
  const query = useQuery({
    queryKey: [...queryKeys.instruments, "detail", id],
    queryFn: () => load(id),
  });
  return {
    view: toScreenState(query.data, () => "filled"),
    retry: useRefresh(),
  };
};

export const useThesis = (
  load: ScreenSources["thesis"],
  id: EntityId,
): {
  readonly view: ScreenState<ThesisView>;
  readonly retry: () => void;
} => {
  const client = useQueryClient();
  const query = useQuery({
    queryKey: [...queryKeys.thesis, id],
    queryFn: () => load(id),
  });
  return {
    view: toScreenState(query.data, (data) =>
      data.claims.length === 0 && data.history.length === 0
        ? "empty"
        : "filled",
    ),
    retry: (): void => {
      detach({ kind: "invalidate-queries", client, keys: [queryKeys.thesis] });
    },
  };
};

export type WatchlistNotice =
  | { readonly kind: "idle" }
  | { readonly kind: "invalid" }
  | { readonly kind: "failed" }
  | { readonly kind: "rejected"; readonly reason: FreeText };

const noticeOf = (
  outcome: WatchlistChange | "failed" | undefined,
): WatchlistNotice => {
  if (outcome === undefined) return { kind: "idle" };
  if (outcome === "failed") return { kind: "failed" };
  return outcome.kind === "rejected"
    ? { kind: "rejected", reason: outcome.reason }
    : { kind: "idle" };
};

export const useWatchlistActions = (
  sources: Pick<ScreenSources, "addWatchlist" | "removeWatchlist">,
): {
  readonly add: (input: InputValue) => void;
  readonly remove: (instrument: EntityId) => void;
  readonly notice: WatchlistNotice;
} => {
  const refresh = useRefresh();
  const add = useMutation({
    mutationFn: async (input: InputValue) => {
      const ticker = parseTicker(input);
      if (ticker.kind === "err") return "invalid" as const;
      const result = await sources.addWatchlist(ticker.value);
      return result.kind === "ok" ? result.value : ("failed" as const);
    },
    onSettled: refresh,
  });
  const remove = useMutation({
    mutationFn: async (instrument: EntityId) => {
      const result = await sources.removeWatchlist(instrument);
      return result.kind === "ok" ? result.value : ("failed" as const);
    },
    onSettled: refresh,
  });
  const latest = add.data ?? remove.data;
  return {
    add: (input): void => {
      add.mutate(input);
    },
    remove: (instrument): void => {
      remove.mutate(instrument);
    },
    notice: latest === "invalid" ? { kind: "invalid" } : noticeOf(latest),
  };
};
