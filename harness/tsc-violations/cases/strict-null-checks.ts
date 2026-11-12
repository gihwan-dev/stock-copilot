export const read = (holder: { readonly value: "v" } | null): "v" =>
  holder.value;
