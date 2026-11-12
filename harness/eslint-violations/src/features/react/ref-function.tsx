import type { ReactElement } from "react";
export const Panel = (): ReactElement => (
  <div
    ref={function attach(node): void {
      void node;
    }}
  />
);
