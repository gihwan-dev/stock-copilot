import * as M from "../../shared/lib/money";
const get = <O, K extends keyof O>(o: O, k: K): O[K] => o[k];
const name = "moneyKey";
export const larger = (a: M.Money, b: M.Money): "x" | "y" =>
  get(a, M[name]) > get(b, M[name]) ? "x" : "y";
