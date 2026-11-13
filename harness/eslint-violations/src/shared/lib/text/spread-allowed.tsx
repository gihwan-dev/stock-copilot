export const spread = (props: { readonly id: "a" }): { readonly id: "a" } => ({
  ...props,
});
