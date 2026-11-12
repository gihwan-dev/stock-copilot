import type { ReactElement } from "react";
export const Spread = (props: { readonly id: "a" }): ReactElement => (
  // eslint-disable-next-line guard/no-jsx-spread -- 끌 수 없는 규칙
  <i {...props} />
);
