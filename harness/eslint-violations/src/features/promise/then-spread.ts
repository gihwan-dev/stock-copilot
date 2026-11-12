type Handler = (value: "a") => "a";
export const recover = (
  pending: Promise<"a">,
  handlers: readonly [Handler],
): Promise<"a"> => pending.then(...handlers);
