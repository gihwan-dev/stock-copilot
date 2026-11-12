export const pick = (
  items: readonly ("a" | "b")[],
  index: 0 | 1,
): "a" | "b" | undefined => items[index];
