type Box = { readonly a: "a"; readonly b: "b" };
export const read = <K extends keyof Box>(box: Box, key: K): "x" => {
  const { [key]: value } = box;
  return value === undefined ? "x" : "x";
};
