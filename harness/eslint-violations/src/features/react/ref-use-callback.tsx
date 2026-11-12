import { useCallback, type ReactElement } from "react";
export const Panel = (): ReactElement => {
  const attach = useCallback((node: HTMLDivElement | null): void => {
    void node;
  }, []);
  return <div ref={useCallback(attach, [attach])} />;
};
