import type { Result } from "../lib";
import type { IpcFailure } from "./schema";

export type ScreenState<T> =
  | { readonly status: "loading" }
  | { readonly status: "empty" }
  | { readonly status: "error"; readonly failure: IpcFailure }
  | { readonly status: "ready"; readonly data: T };

export type Fill = "empty" | "filled";

export const toScreenState = <T>(
  result: Result<T, IpcFailure> | undefined,
  fill: (data: T) => Fill,
): ScreenState<T> => {
  if (result === undefined) return { status: "loading" };
  if (result.kind === "err") {
    const failure = result.error;
    return failure.kind === "command" && failure.error.kind === "no-data"
      ? { status: "empty" }
      : { status: "error", failure };
  }
  return fill(result.value) === "empty"
    ? { status: "empty" }
    : { status: "ready", data: result.value };
};
