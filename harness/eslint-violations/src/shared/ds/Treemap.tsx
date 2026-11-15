export const probe = (): "one" | "other" =>
  Number("1") === 1 ? "one" : "other";
