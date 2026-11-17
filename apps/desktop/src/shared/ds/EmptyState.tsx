import type { Action } from "./control-types";
import { Button } from "./Button";
import type { ReactElement } from "react";
import type { UiText } from "../lib";
import type { IconName } from "./icon-paths";
import { Icon } from "./Icon";
type Props = {
  readonly title: UiText;
  readonly icon?: IconName;
  readonly children?: UiText;
  readonly action?: Action;
};
export const EmptyState = (props: Props): ReactElement => (
  <div className="flex flex-col items-center gap-space-2 py-space-10 px-space-6 text-center text-ink-muted">
    <span className="text-ink-subtle">
      <Icon name={props.icon ?? "info"} size="icon-lg" />
    </span>
    <p className="text-body font-semibold text-ink">{props.title}</p>
    {props.children === undefined ? null : (
      <p className="text-body-sm">{props.children}</p>
    )}
    {props.action === undefined ? null : (
      <div className="mt-space-2">
        <Button
          size="control-height-sm"
          onClick={props.action.onPress}
          disabled={props.action.disabled ?? "enabled"}
        >
          {props.action.label}
        </Button>
      </div>
    )}
  </div>
);
