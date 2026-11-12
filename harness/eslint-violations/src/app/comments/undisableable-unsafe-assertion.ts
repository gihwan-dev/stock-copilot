export const pick = (value: "a" | "b"): "a" =>
  // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- 끌 수 없는 규칙
  value as "a";
