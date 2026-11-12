export const label = (side: "buy" | "sell"): "b" | "s" => {
  switch (side) {
    case "buy":
      return "b";
    case "sell":
      return "s";
  }
};
