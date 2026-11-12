export const fail = (): Promise<"a"> => Promise.reject(new Error("x"));
