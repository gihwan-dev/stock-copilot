type StringBrand = string & { readonly brand: "Money" };
export const total = (a: StringBrand, b: StringBrand): "x" =>
  parseFloat(a) + Number(b) === 0 && a + b === "" ? "x" : "x";
