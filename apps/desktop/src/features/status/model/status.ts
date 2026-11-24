import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  toScreenState,
  type ScreenSources,
  type ScreenState,
  type StatusView,
} from "../../../shared/ipc";
import { queryKeys, screenKeys } from "../../../shared/lib";
import { detach } from "../../../shared/platform";

export type StatusScreenState = ScreenState<StatusView>;

export const useStatus = (
  load: ScreenSources["status"],
): { readonly view: StatusScreenState; readonly retry: () => void } => {
  const client = useQueryClient();
  const query = useQuery({ queryKey: screenKeys.status, queryFn: load });
  return {
    view: toScreenState(query.data, () => "filled"),
    retry: (): void => {
      detach({ kind: "invalidate-queries", client, keys: [queryKeys.status] });
    },
  };
};
