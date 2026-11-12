export const run = (target: { a: "x" | "y" }): "y" => {
  target.a = "y";
  return "y";
};
