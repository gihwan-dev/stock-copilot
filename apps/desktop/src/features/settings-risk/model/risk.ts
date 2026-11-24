import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  toScreenState,
  type CardExpiry,
  type ExcludableType,
  type HorizonOption,
  type InstrumentCap,
  type IpcFailure,
  type OrderCountLimit,
  type RiskAppetiteOption,
  type RiskSaveOutcome,
  type ScreenState,
  type SectorCap,
} from "../../../shared/ipc";
import {
  emptyInputValue,
  parseInputValue,
  queryKeys,
  type InputValue,
  type Result,
  type UiText,
} from "../../../shared/lib";
import { detach } from "../../../shared/platform";

export type Choice = { readonly value: InputValue; readonly label: UiText };

type Literal =
  | HorizonOption
  | RiskAppetiteOption
  | InstrumentCap
  | SectorCap
  | ExcludableType
  | CardExpiry
  | OrderCountLimit;

export const choicesOf = <T extends Literal>(
  values: readonly T[],
  label: (value: T) => UiText,
): readonly Choice[] =>
  values.flatMap((value): readonly Choice[] => {
    const input = parseInputValue(value);
    return input.kind === "ok"
      ? [{ value: input.value, label: label(value) }]
      : [];
  });

export const inputOf = (value: Literal): InputValue => {
  const input = parseInputValue(value);
  return input.kind === "ok" ? input.value : emptyInputValue;
};

export const pick = <T extends Literal>(
  values: readonly T[],
  input: InputValue,
): T | undefined => values.find((value) => inputOf(value) === input);

export type SaveNotice = RiskSaveOutcome["kind"] | "idle" | "failed";

export const useDraftSource = <T>(
  key: readonly ["settings", "risk", "profile" | "order"],
  load: () => Promise<Result<T, IpcFailure>>,
): { readonly view: ScreenState<T>; readonly retry: () => void } => {
  const client = useQueryClient();
  const query = useQuery({ queryKey: key, queryFn: load });
  return {
    view: toScreenState(query.data, () => "filled"),
    retry: (): void => {
      detach({
        kind: "invalidate-queries",
        client,
        keys: [queryKeys.settings],
      });
    },
  };
};

const noticeOf = (
  result: Result<RiskSaveOutcome, IpcFailure> | undefined,
  status: "idle" | "pending" | "error" | "success",
): SaveNotice => {
  if (result === undefined) return status === "error" ? "failed" : "idle";
  return result.kind === "ok" ? result.value.kind : "failed";
};

export const useSave = <T>(
  save: (draft: T) => Promise<Result<RiskSaveOutcome, IpcFailure>>,
): {
  readonly submit: (draft: T) => void;
  readonly notice: SaveNotice;
  readonly reason: RiskSaveOutcome | undefined;
} => {
  const client = useQueryClient();
  const mutation = useMutation({
    mutationFn: save,
    onSettled: (): void => {
      detach({
        kind: "invalidate-queries",
        client,
        keys: [queryKeys.settings],
      });
    },
  });
  const result = mutation.data;
  const notice = noticeOf(result, mutation.status);
  return {
    submit: (draft): void => {
      mutation.mutate(draft);
    },
    notice,
    reason: result?.kind === "ok" ? result.value : undefined,
  };
};
