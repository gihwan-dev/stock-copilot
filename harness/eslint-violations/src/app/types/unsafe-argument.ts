const take = (value: readonly unknown[]): unknown => value;
export const run = (input: unknown): unknown => take(JSON.parse(String(input)));
