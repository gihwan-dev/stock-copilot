import { useRef, type ReactElement } from "react";
export const Panel = (props: Readonly<{ mode: "a" | "b" }>): ReactElement => {
  const first = useRef<HTMLDivElement>(null);
  const second = useRef<HTMLDivElement>(null);
  return <div ref={props.mode === "a" ? first : second} />;
};
