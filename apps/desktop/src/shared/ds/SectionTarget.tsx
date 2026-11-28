import type { ReactElement } from "react";
import type { EntityId } from "../lib";

export type SectionPosition = {
  readonly topic: "holdings" | "candidates" | "economy";
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
      tabIndex={-1}
    >
      {props.children}
    </div>
  );
};
