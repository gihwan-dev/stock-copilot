export const recover = (pending: Promise<"a">): Promise<"a" | "b"> =>
  pending["catch"]((): "b" => "b");
