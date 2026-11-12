export const keys = (target: Readonly<{ a: "x" }>): unknown =>
  Object.getOwnPropertyDescriptor(target, "a");
