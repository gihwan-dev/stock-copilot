import type { ReactElement } from "react";
const attach = (node: unknown): undefined =>
  node === null ? undefined : undefined;
export const Panel = (): ReactElement => <div ref={attach} />;
