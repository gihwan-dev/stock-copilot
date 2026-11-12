export const run = (target: Readonly<{ a: "x" }>): unknown =>
  Object.setPrototypeOf(target, null);
