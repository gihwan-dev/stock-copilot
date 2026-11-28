import type { ReactElement } from "react";
import type { EntityId } from "../lib";

type Target = "holdings" | "candidates" | "economy";

export const SectionLink = (props: {
  readonly children: ReactElement;
  readonly onPress: () => void;
}): ReactElement => (
  <button
    type="button"
    className="markdown-link text-body-sm focus-visible:outline-solid focus-visible:outline-focus"
    onClick={props.onPress}
  >
    {props.children}
  </button>
);

export const SectionTarget = (props: {
  readonly target: Target;
  readonly source?: EntityId;
  readonly children: ReactElement;
}): ReactElement => (
  <div
    id={
      props.source === undefined
        ? props.target
        : `${props.target}-${props.source}`
    }
    tabIndex={-1}
  >
    {props.children}
  </div>
);
