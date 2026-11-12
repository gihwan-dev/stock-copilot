export const recover = (pending: Promise<"a">): Promise<"a" | "b"> =>
  // eslint-disable-next-line local/no-promise-catch -- 끌 수 없는 규칙
  pending.catch((): "b" => "b");
