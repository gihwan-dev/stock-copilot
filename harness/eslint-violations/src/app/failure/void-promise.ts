const load = async (): Promise<"a"> => Promise.resolve("a");
export const run = (): "b" => {
  void load();
  return "b";
};
