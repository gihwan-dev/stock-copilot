export const recover = (pending: Promise<"a">): Promise<"a" | "b"> =>
  pending.then(
    (value) => value,
    (): "b" => "b",
  );
