import type { ReactElement } from "react";
import type { EntityId } from "../lib";

export type SectionPosition = {
  readonly topic:
    | "holdings"
    | "candidates"
    | "economy"
    | "positions"
    | "daily-briefing"
    | "signals"
    | "allocation";
  readonly source?: EntityId;
};
export const SectionTarget = (props: {
  readonly position: SectionPosition | undefined;
  readonly children: ReactElement;
}): ReactElement => {
  const target = props.position;
  if (target === undefined) return props.children;
  return (
    <div
      id={
        target.source === undefined
          ? target.topic
          : `${target.topic}-${target.source}`
      }
      className={
        target.source === undefined
          ? "max-w-full"
          : "rounded-md px-space-2 py-space-1 focus:bg-surface-sunken focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring"
      }
      tabIndex={-1}
    >
      {props.children}
    </div>
  );
};
