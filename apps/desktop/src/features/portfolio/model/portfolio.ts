import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  toScreenState,
  type AllocationView,
  type PortfolioView,
  type ScreenSources,
  type ScreenState,
} from "../../../shared/ipc";
import { queryKeys, screenKeys } from "../../../shared/lib";
import { detach } from "../../../shared/platform";

export type PortfolioScreenState = ScreenState<PortfolioView>;

export type AllocationScreenState = ScreenState<AllocationView>;

export const usePortfolio = (
  load: ScreenSources["portfolio"],
): { readonly view: PortfolioScreenState; readonly retry: () => void } => {
  const client = useQueryClient();
  const query = useQuery({ queryKey: screenKeys.portfolio, queryFn: load });
  return {
    view: toScreenState(query.data, () => "filled"),
    retry: (): void => {
      detach({
        kind: "invalidate-queries",
        client,
        keys: [queryKeys.portfolio, queryKeys.allocation],
      });
    },
  };
};

export const useAllocation = (
  load: ScreenSources["allocation"],
): AllocationScreenState => {
  const query = useQuery({ queryKey: screenKeys.allocation, queryFn: load });
  return toScreenState(query.data, (data) =>
    data.instruments.length === 0 ? "empty" : "filled",
  );
};
