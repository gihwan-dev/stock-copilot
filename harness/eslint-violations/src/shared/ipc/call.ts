export const settle = <T>(pending: Promise<T>): Promise<T | undefined> =>
  pending.then(
    (value) => value,
    () => undefined,
  );
export const recover = <T>(pending: Promise<T>): Promise<T | undefined> =>
  pending.catch(() => undefined);
