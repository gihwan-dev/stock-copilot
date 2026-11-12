export const handler = (pending: Promise<"a">): "x" => {
  const { catch: handle } = pending;
  return handle === undefined ? "x" : "x";
};
