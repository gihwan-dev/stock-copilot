export const check = (value: true | false | null): "yes" | "no" =>
  value ? "yes" : "no";
