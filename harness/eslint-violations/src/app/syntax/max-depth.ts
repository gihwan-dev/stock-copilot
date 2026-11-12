export const deep = (
  a: "on" | "off",
  b: "on" | "off",
  c: "on" | "off",
): "x" | "y" => {
  if (a === "on") {
    if (b === "on") {
      if (c === "on") {
        if (a === b) {
          return "x";
        }
      }
    }
  }
  return "y";
};
