import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import type {
  AliasRow,
  ContactText,
  IpcFailure,
  NewsSettings,
  OptionalKey,
  SecretText,
  SettingsClient,
  SourceSetting,
  WakeCriteria,
} from "../../../shared/ipc";
import type { Result } from "../../../shared/lib";

const newsKey = ["news-settings"] as const;

type NewsReply = Result<NewsSettings, IpcFailure>;

export type NewsView =
  | { readonly status: "loading" }
  | { readonly status: "error" }
  | { readonly status: "ready"; readonly data: NewsSettings };

export const useNewsSettings = (client: SettingsClient): NewsView => {
  const query = useQuery({ queryKey: newsKey, queryFn: client.newsSettings });
  if (query.data === undefined) return { status: "loading" };
  if (query.data.kind === "err") return { status: "error" };
  return { status: "ready", data: query.data.value };
};

export type NewsChange =
  | { readonly kind: "source"; readonly setting: SourceSetting }
  | { readonly kind: "contact"; readonly contact: ContactText | null }
  | { readonly kind: "wake"; readonly wake: WakeCriteria }
  | { readonly kind: "aliases"; readonly aliases: readonly AliasRow[] };

export type SaveState = "idle" | "pending" | "saved" | "failed";

const applyNews = (
  client: SettingsClient,
  change: NewsChange,
): Promise<NewsReply> => {
  switch (change.kind) {
    case "source":
      return client.setNewsSource(change.setting);
    case "contact":
      return client.setSecContact(change.contact);
    case "wake":
      return client.setWakeCriteria(change.wake);
    case "aliases":
      return client.setAliases(change.aliases);
  }
};

const stateOf = (
  pending: "pending" | "settled",
  reply: "none" | "ok" | "err",
): SaveState => {
  if (pending === "pending") return "pending";
  if (reply === "none") return "idle";
  return reply === "ok" ? "saved" : "failed";
};

export type NewsSaver = {
  readonly state: SaveState;
  readonly save: (change: NewsChange) => void;
};

export const useNewsSaver = (client: SettingsClient): NewsSaver => {
  const queryClient = useQueryClient();
  const mutation = useMutation({
    mutationFn: (change: NewsChange) => applyNews(client, change),
    onSuccess: (reply): NewsReply | undefined =>
      reply.kind === "ok"
        ? queryClient.setQueryData<NewsReply>(newsKey, reply)
        : undefined,
  });
  return {
    state: stateOf(
      mutation.isPending ? "pending" : "settled",
      mutation.data?.kind ?? "none",
    ),
    save: (change): void => {
      mutation.mutate(change);
    },
  };
};

export type KeyEntry = {
  readonly key: OptionalKey;
  readonly secret: SecretText;
};

export type KeySaver = {
  readonly state: SaveState;
  readonly save: (entry: KeyEntry) => void;
};

export const useKeySaver = (client: SettingsClient): KeySaver => {
  const queryClient = useQueryClient();
  const [outcome, setOutcome] = useState<"none" | "ok" | "err">("none");
  const mutation = useMutation({
    mutationFn: (entry: KeyEntry) =>
      client.setOptionalKey(entry.key, entry.secret),
    gcTime: 0,
    onSuccess: (reply): NewsReply | undefined =>
      reply.kind === "ok"
        ? queryClient.setQueryData<NewsReply>(newsKey, reply)
        : undefined,
  });
  return {
    state: stateOf(mutation.isPending ? "pending" : "settled", outcome),
    save: (entry): void => {
      setOutcome("none");
      mutation.mutate(entry, {
        onSettled: (reply): void => {
          setOutcome(reply?.kind === "ok" ? "ok" : "err");
          mutation.reset();
        },
      });
    },
  };
};
