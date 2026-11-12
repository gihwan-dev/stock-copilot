export const settleAll = (
  pending: Promise<"a">,
): Promise<readonly PromiseSettledResult<"a">[]> =>
  Promise.allSettled([pending]);
