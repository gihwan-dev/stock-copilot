const load = async (): Promise<"a"> => Promise.resolve("a");
export const run = (): "b" => {
  load();
  return "b";
};
