import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  openExternal,
  type AiSettings,
  type AiTaskSetting,
  type BudgetPercent,
  type ConfigPathText,
  type IpcFailure,
  type OpenExternalResult,
  type ScheduledJobSetting,
  type ScheduledJobs,
  type SettingsClient,
} from "../../../shared/ipc";
import { err, parseExternalUrl, type Result } from "../../../shared/lib";

const aiKey = ["ai-settings"] as const;
const jobsKey = ["scheduled-jobs"] as const;

type AiReply = Result<AiSettings, IpcFailure>;
type JobsReply = Result<ScheduledJobs, IpcFailure>;

type Loaded<T> =
  | { readonly status: "loading" }
  | { readonly status: "error" }
  | { readonly status: "ready"; readonly data: T };

const loaded = <T>(data: Result<T, IpcFailure> | undefined): Loaded<T> => {
  if (data === undefined) return { status: "loading" };
  if (data.kind === "err") return { status: "error" };
  return { status: "ready", data: data.value };
};

export const useAiSettings = (client: SettingsClient): Loaded<AiSettings> =>
  loaded(useQuery({ queryKey: aiKey, queryFn: client.aiSettings }).data);

export const useScheduledJobs = (
  client: SettingsClient,
): Loaded<ScheduledJobs> =>
  loaded(useQuery({ queryKey: jobsKey, queryFn: client.scheduledJobs }).data);

export type AiChange =
  | { readonly kind: "task"; readonly setting: AiTaskSetting }
  | { readonly kind: "budget"; readonly percent: BudgetPercent }
  | { readonly kind: "config"; readonly path: ConfigPathText | null };

export type SaveState = "idle" | "pending" | "saved" | "failed";

export type Saver<C> = {
  readonly state: SaveState;
  readonly save: (change: C) => void;
};

const saveState = (
  pending: "pending" | "settled",
  reply: "none" | "ok" | "err",
): SaveState => {
  if (pending === "pending") return "pending";
  if (reply === "none") return "idle";
  return reply === "ok" ? "saved" : "failed";
};

const applyAi = (
  client: SettingsClient,
  change: AiChange,
): Promise<AiReply> => {
  switch (change.kind) {
    case "task":
      return client.setTaskModel(change.setting);
    case "budget":
      return client.setBudgetPercent(change.percent);
    case "config":
      return client.setClaudeConfigDir(change.path);
  }
};

export const useAiSaver = (client: SettingsClient): Saver<AiChange> => {
  const queryClient = useQueryClient();
  const mutation = useMutation({
    mutationFn: (change: AiChange) => applyAi(client, change),
    onSuccess: (reply): AiReply | undefined =>
      reply.kind === "ok"
        ? queryClient.setQueryData<AiReply>(aiKey, reply)
        : undefined,
  });
  return {
    state: saveState(
      mutation.isPending ? "pending" : "settled",
      mutation.data?.kind ?? "none",
    ),
    save: (change): void => {
      mutation.mutate(change);
    },
  };
};

export const useJobSaver = (
  client: SettingsClient,
): Saver<ScheduledJobSetting> => {
  const queryClient = useQueryClient();
  const mutation = useMutation({
    mutationFn: client.setScheduledJob,
    onSuccess: (reply): JobsReply | undefined =>
      reply.kind === "ok"
        ? queryClient.setQueryData<JobsReply>(jobsKey, reply)
        : undefined,
  });
  return {
    state: saveState(
      mutation.isPending ? "pending" : "settled",
      mutation.data?.kind ?? "none",
    ),
    save: (setting): void => {
      mutation.mutate(setting);
    },
  };
};

const badUrl: IpcFailure = { kind: "invalid-response" };

const privacyUrl = parseExternalUrl(
  "https://claude.ai/settings/data-privacy-controls",
);

export type LinkOpener = {
  readonly state: "idle" | "opened" | "failed";
  readonly open: () => void;
};

export const usePrivacyLink = (): LinkOpener => {
  const mutation = useMutation({
    mutationFn: (): Promise<Result<OpenExternalResult, IpcFailure>> =>
      privacyUrl.kind === "ok"
        ? openExternal(privacyUrl.value)
        : Promise.resolve(err(badUrl)),
  });
  const done = mutation.data?.kind === "ok" ? "opened" : ("failed" as const);
  return {
    state: mutation.data === undefined ? "idle" : done,
    open: (): void => {
      mutation.mutate();
    },
  };
};
