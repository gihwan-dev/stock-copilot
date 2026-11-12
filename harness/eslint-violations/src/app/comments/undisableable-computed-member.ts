export const pick = (
  items: readonly ("a" | "b")[],
  index: 0 | 1,
): "a" | "b" | undefined =>
  // eslint-disable-next-line local/no-computed-member -- 끌 수 없는 규칙
  items[index];
