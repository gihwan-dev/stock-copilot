export const check = (value: Readonly<{ a: "x" }> | null): "yes" | "no" =>
  value ? "yes" : "no";
