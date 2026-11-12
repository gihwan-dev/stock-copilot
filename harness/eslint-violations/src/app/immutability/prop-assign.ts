export const run = (): Readonly<{ a: "x" | "y" }> => {
  const target: { a: "x" | "y" } = { a: "x" };
  target.a = "y";
  return target;
};
