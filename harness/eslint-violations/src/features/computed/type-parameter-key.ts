type Box = { readonly a: "a"; readonly b: "b" };
export const read = <K extends keyof Box>(box: Box, key: K): Box[K] => box[key];
