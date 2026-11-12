export const run = (): Readonly<{ a?: "x" }> => {
  const target: { a?: "x" } = { a: "x" };
  delete target.a;
  return target;
};
