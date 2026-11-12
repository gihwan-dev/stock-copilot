const pick = (value: "a"): "a" => value;
export const run = (): unknown => pick.apply(undefined, ["a"]);
