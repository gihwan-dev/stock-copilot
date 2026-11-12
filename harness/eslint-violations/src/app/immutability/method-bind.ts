const pick = (value: "a"): "a" => value;
export const run = pick.bind(undefined);
