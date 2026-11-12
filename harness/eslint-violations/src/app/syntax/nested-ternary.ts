export const pick = (value: "a" | "b" | "c"): "x" | "y" | "z" =>
  value === "a" ? "x" : value === "b" ? "y" : "z";
