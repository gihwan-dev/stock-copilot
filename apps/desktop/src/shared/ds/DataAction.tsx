import { useId, type ReactElement } from "react";
import type { UiText } from "../lib";
import { Badge } from "./Badge";
import { Icon } from "./Icon";

export const DataAction = (props: {
  readonly onPress: () => void;
  readonly children: ReactElement;
  readonly alert: UiText | undefined;
}): ReactElement => {
  const nameId = useId();
  const statusId = useId();
  return (
    <button
      type="button"
      aria-labelledby={nameId}
      aria-describedby={props.alert === undefined ? undefined : statusId}
      onClick={props.onPress}
      className="inline-flex items-center justify-between gap-space-2 w-full max-w-full text-left text-ink-muted rounded-sm hover:text-primary focus-visible:outline-solid focus-visible:outline-focus"
    >
      <span className="inline-flex items-center gap-space-2 flex-wrap">
        <span id={nameId}>{props.children}</span>
        {props.alert === undefined ? null : (
          <span id={statusId} className="inline-flex">
            <Badge tone="alert" outline="visible">
              {props.alert}
            </Badge>
          </span>
        )}
      </span>
      <Icon name="chevron-right" size="icon-sm" />
    </button>
  );
};
