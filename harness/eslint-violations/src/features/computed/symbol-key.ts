const key: unique symbol = Symbol("k");
type Box = { readonly [key]: "v" };
export const read = (box: Box): "v" => box[key];
