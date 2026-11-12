type Schema = Readonly<{
  parse: (value: unknown) => "a";
  parseAsync: (value: unknown) => Promise<"a">;
}>;
export const run = (s: Schema): unknown => s.parseAsync(1);
