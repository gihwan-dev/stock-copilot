export const pick = (value: "a" | "b"): "a" =>
  // eslint-disable-next-line @typescript-eslint/consistent-type-assertions -- 끌 수 없는 규칙
  <"a">value;
