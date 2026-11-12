export const grade = (
  value: "a" | "b" | "c" | "d" | "e" | "f" | "g" | "h" | "i",
): "x" | "y" => {
  if (value === "a") return "x";
  if (value === "b") return "x";
  if (value === "c") return "x";
  if (value === "d") return "x";
  if (value === "e") return "x";
  if (value === "f") return "x";
  if (value === "g") return "x";
  if (value === "h") return "x";
  return "y";
};
