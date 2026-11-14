import type { ReactElement } from "react";
import type { UiText, EntityId } from "../lib";
type Props = {
  readonly children: UiText;
  readonly level?: 1 | 2 | 3;
  readonly id?: EntityId;
};
export const Heading = (props: Props): ReactElement => {
  if (props.level === 1)
    return (
      <h1 id={props.id} className="text-title-lg text-ink">
        {props.children}
      </h1>
    );
  if (props.level === 3)
    return (
      <h3 id={props.id} className="text-title-sm text-ink">
        {props.children}
      </h3>
    );
  return (
    <h2 id={props.id} className="text-title-md text-ink">
      {props.children}
    </h2>
  );
};
