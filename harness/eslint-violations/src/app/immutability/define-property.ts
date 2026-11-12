export const run = (target: Readonly<{ a?: "x" }>): unknown =>
  Object.defineProperty(target, "a", { value: "x" });
