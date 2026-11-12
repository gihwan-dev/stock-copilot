const load = async (): Promise<"a"> => Promise.resolve("a");
export const run = (): "yes" | "no" => (load() ? "yes" : "no");
