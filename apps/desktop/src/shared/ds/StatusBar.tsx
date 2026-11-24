import type { Action } from "./control-types";
import type { ReactElement } from "react";
import type { UiText } from "../lib";
import { Button } from "./Button";
import { Icon } from "./Icon";
import type { IconName } from "./icon-paths";
type Tone = "warning" | "error" | "info";
type Props = {
  readonly tone?: Tone;
  readonly children?: UiText;
  readonly action?: Action;
};
const styles = {
  warning:
    "flex flex-wrap items-center gap-space-2 min-h-control-height py-space-1 px-space-4 text-size-md leading-md bg-alert-soft text-ink border-b-hairline border-solid border-alert",
  error:
    "flex flex-wrap items-center gap-space-2 min-h-control-height py-space-1 px-space-4 text-size-md leading-md bg-alert-soft text-ink border-b-hairline border-solid border-alert",
  info: "flex flex-wrap items-center gap-space-2 min-h-control-height py-space-1 px-space-4 text-size-md leading-md bg-surface-sunken text-ink border-b-hairline border-solid border-line",
} as const;
const statusIcon = (tone: Tone): IconName => {
  switch (tone) {
    case "warning":
      return "triangle-alert";
    case "error":
      return "circle-x";
    case "info":
      return "info";
  }
};
const statusAction = (action: Props["action"]): ReactElement | null =>
  action === undefined ? null : (
    <Button
      size="control-height-sm"
      onClick={action.onPress}
      disabled={action.disabled ?? "enabled"}
    >
      {action.label}
    </Button>
  );
const statusIndicator = (tone: Tone): ReactElement => (
  <span
    className={
      tone === "info"
        ? "flex-none text-ink-muted mr-space-2"
        : "flex-none text-alert mr-space-2"
    }
  >
    <Icon name={statusIcon(tone)} />
  </span>
);
const emptyStyles = {
  warning: "flex items-center",
  error: "flex items-center",
  info: "flex items-center",
} as const;
const statusClasses = {
  warning: "flex-1 status-sentence",
  info: "flex-1 status-sentence",
  error: "status-sentence",
} as const;
const alertClasses = {
  warning: "status-sentence",
  info: "status-sentence",
  error: "flex-1 status-sentence",
} as const;
export const StatusBar = (props: Props): ReactElement => {
  const tone = props.tone ?? "warning";
  const present = props.children !== undefined && props.children.length > 0;
  return (
    <div className="flex-none">
      <div className={present ? styles[tone] : emptyStyles[tone]}>
        <div className="flex-1 flex items-center wrap-slot">
          {present ? statusIndicator(tone) : null}
          <div role="status" aria-atomic="true" className={statusClasses[tone]}>
            {tone === "error" ? undefined : props.children}
          </div>
          <div role="alert" aria-atomic="true" className={alertClasses[tone]}>
            {tone === "error" ? props.children : undefined}
          </div>
        </div>
        {present ? statusAction(props.action) : null}
      </div>
    </div>
  );
};
