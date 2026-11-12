export const orElse = <T>(
  value: T | undefined,
  fallback: NonNullable<T>,
): NonNullable<T> => value ?? fallback;
