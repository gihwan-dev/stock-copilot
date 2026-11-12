export const next = (pending: Promise<"a">): Promise<"b"> =>
  pending.then((): "b" => "b");
