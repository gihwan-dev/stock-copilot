export const run = (input: Readonly<{ fn: unknown }>): unknown => {
  const loose = input as unknown as { readonly fn: any };
  return loose.fn();
};
