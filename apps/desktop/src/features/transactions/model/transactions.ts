import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  toScreenState,
  type ScreenSources,
  type ScreenState,
  type TransactionsView,
} from "../../../shared/ipc";
import { queryKeys, screenKeys } from "../../../shared/lib";
import { detach } from "../../../shared/platform";

export type TransactionsScreenState = ScreenState<TransactionsView>;

export const useTransactions = (
  load: ScreenSources["transactions"],
): { readonly view: TransactionsScreenState; readonly retry: () => void } => {
  const client = useQueryClient();
  const query = useQuery({ queryKey: screenKeys.transactions, queryFn: load });
  return {
    view: toScreenState(query.data, (data) =>
      data.rows.length === 0 ? "empty" : "filled",
    ),
    retry: (): void => {
      detach({
        kind: "invalidate-queries",
        client,
        keys: [queryKeys.transactions],
      });
    },
  };
};
