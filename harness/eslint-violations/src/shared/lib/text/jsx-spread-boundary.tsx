import type { ReactElement } from "react";
export const Spread = (props: { readonly id: "a" }): ReactElement => (
  <span {...props} />
);
