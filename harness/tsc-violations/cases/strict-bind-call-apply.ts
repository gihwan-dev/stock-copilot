const only = (input: "a"): "a" => input;
export const called = only.call(undefined, "b");
