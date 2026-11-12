const step = (["a"] as const).values().next();
export const value: "a" = step.value;
