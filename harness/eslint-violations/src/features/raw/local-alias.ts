type Plain = string;
export const echo = (value: Plain): "x" => (value === "a" ? "x" : "x");
