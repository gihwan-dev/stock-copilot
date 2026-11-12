type Schema = Readonly<{ parseAsync: (value: unknown) => Promise<"a"> }>;
export const run = (s: Schema): unknown => {
  const { parseAsync } = s;
  return parseAsync(1);
};
