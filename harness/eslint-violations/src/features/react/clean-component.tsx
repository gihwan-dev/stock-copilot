import { useMemo, useRef, useState, type ReactElement } from "react";
export const Counter = (props: {
  readonly label: "합계" | "평균";
}): ReactElement => {
  const [mode, setMode] = useState<"open" | "closed">("closed");
  const nodeRef = useRef<HTMLDivElement>(null);
  const title = useMemo(() => props.label, [props.label]);
  return (
    <div ref={nodeRef}>
      <button
        type="button"
        onClick={(): void => {
          setMode(mode === "open" ? "closed" : "open");
        }}
      >
        {title}
      </button>
    </div>
  );
};
