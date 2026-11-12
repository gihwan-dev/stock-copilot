export type Ok<T> = { readonly kind: "ok"; readonly value: T };

export type Err<E> = { readonly kind: "err"; readonly error: E };

export type Result<T, E> = Ok<T> | Err<E>;

export const ok = <T>(value: T): Ok<T> => ({ kind: "ok", value });

export const err = <E>(error: E): Err<E> => ({ kind: "err", error });
