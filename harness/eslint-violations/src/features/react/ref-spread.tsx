import { useRef, type ReactElement } from "react";
export const Panel = (): ReactElement => {
  const node = useRef<HTMLDivElement>(null);
  return <div {...{ ref: node }} />;
};
