import { err, ok, type Result } from "../lib";

export type IpcFailure = { readonly kind: "rejected" };

const rejected = (): Result<never, IpcFailure> => err({ kind: "rejected" });

export const settle = <T>(
  pending: Promise<T>,
): Promise<Result<T, IpcFailure>> => pending.then(ok, rejected);
