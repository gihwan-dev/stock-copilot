const compute = (): "a" => "a";
export const run = (): "b" => {
  compute();
  return "b";
};
