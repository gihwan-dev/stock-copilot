const compute = (): "a" => "a";
export const run = (): "b" => {
  const _ignored = compute();
  return "b";
};
