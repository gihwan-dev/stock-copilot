export const read = (input: Readonly<{ fn: unknown }>): readonly unknown[] => {
  const loose = input as unknown as { readonly fn: any };
  return loose.fn;
};
