import type { Money } from "../../shared/lib/money";
export const pick = (a: Money, b: Money): "x" | "y" => {
  switch (a) {
    case b:
      return "x";
  }
  return "y";
};
