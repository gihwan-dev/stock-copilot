export function pick(value: "a"): "a";
export function pick(value: "a" | "b"): "a" | "b" {
  return value;
}
