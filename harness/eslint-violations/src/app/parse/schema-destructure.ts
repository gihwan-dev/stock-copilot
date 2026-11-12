type Schema = Readonly<{ parse: (value: unknown) => "a" }>;
export const run = (s: Schema): unknown => {
  const { parse } = s;
  return parse(1);
};
