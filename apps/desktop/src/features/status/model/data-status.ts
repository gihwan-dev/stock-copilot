import { useQuery } from "@tanstack/react-query";
import type { DataStatusView, IpcFailure } from "../../../shared/ipc";
import { queryKeys, type Result } from "../../../shared/lib";

const refreshMs = 60_000;

export type DataStatusState =
  | { readonly status: "loading" }
  | { readonly status: "error" }
  | { readonly status: "ready"; readonly data: DataStatusView };

export const useDataStatus = (
  load: () => Promise<Result<DataStatusView, IpcFailure>>,
): DataStatusState => {
  const query = useQuery({
    queryKey: [...queryKeys.status, "data"],
    queryFn: load,
    refetchInterval: refreshMs,
  });
  const reply = query.data;
  if (reply === undefined) return { status: "loading" };
  if (reply.kind === "err") return { status: "error" };
  return { status: "ready", data: reply.value };
};
