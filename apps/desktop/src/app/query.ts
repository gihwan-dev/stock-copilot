import { QueryClient, queryOptions, useQuery } from "@tanstack/react-query";
import {
  appVersion,
  type AppVersion,
  type CommandError,
  type IpcFailure,
} from "../shared/ipc";
import { queryKeys, text, type Result, type UiText } from "../shared/lib";

export type QueryView<T> =
  | { readonly status: "loading" }
  | { readonly status: "error"; readonly failure: IpcFailure }
  | { readonly status: "ready"; readonly data: T };

export const createAppQueryClient = (): QueryClient =>
  new QueryClient({
    defaultOptions: {
      queries: {
        retry: false,
        refetchOnWindowFocus: false,
        networkMode: "always",
      },
      mutations: {
        networkMode: "always",
      },
    },
  });

const appVersionOptions = queryOptions({
  queryKey: queryKeys.appVersion,
  queryFn: appVersion,
});

const toQueryView = <T>(
  data: Result<T, IpcFailure> | undefined,
): QueryView<T> => {
  if (data === undefined) return { status: "loading" };
  if (data.kind === "err") return { status: "error", failure: data.error };
  return { status: "ready", data: data.value };
};

export const useAppVersion = (): QueryView<AppVersion> => {
  const query = useQuery(appVersionOptions);
  return toQueryView(query.data);
};

const describeCommandError = (error: CommandError): UiText => {
  switch (error.kind) {
    case "retryable":
      return text("app.ipcRetryable", "ko");
    case "user-fixable":
      return text("app.ipcUserFixable", "ko");
    case "bug":
      return text("app.ipcBug", "ko");
    case "no-data":
      return text("app.ipcNoData", "ko");
  }
};

export const describeIpcFailure = (failure: IpcFailure): UiText => {
  switch (failure.kind) {
    case "timeout":
      return text("app.ipcTimeout", "ko");
    case "framework":
    case "invalid-response":
      return text("app.ipcUnavailable", "ko");
    case "command":
      return describeCommandError(failure.error);
  }
};
