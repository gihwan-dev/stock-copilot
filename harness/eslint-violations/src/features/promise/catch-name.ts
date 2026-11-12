const name = "catch";
export const recover = (pending: Promise<"a">): Promise<"a" | "b"> =>
  pending[name]((): "b" => "b");
