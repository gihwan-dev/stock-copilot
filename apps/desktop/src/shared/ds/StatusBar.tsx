import type { ReactElement } from "react";
import type { UiText } from "../lib";
import { Button } from "./Button";
import { Icon } from "./Icon";
import type { IconName } from "./icon-paths";
type Tone = "warning" | "error" | "info";
type Props = {
  readonly tone?: Tone;
  readonly children?: UiText;
  readonly action?: {
    readonly label: UiText;
    readonly onPress: () => void;
    readonly disabled?: "enabled" | "disabled";
  };
};
const styles = {
  warning:
    "flex-none flex items-center gap-space-2 min-h-control-height py-space-1 px-space-6 text-size-md leading-md bg-alert-soft text-ink border-b-hairline border-solid border-alert",
  error:
    "flex-none flex items-center gap-space-2 min-h-control-height py-space-1 px-space-6 text-size-md leading-md bg-alert-soft text-ink border-b-hairline border-solid border-alert",
  info: "flex-none flex items-center gap-space-2 min-h-control-height py-space-1 px-space-6 text-size-md leading-md bg-surface-sunken text-ink border-b-hairline border-solid border-line",
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
export const StatusBar = (props: Props): ReactElement | null => {
  const tone = props.tone ?? "warning";

  const content =
    props.children === undefined || props.children.length === 0 ? null : (
      <div className={styles[tone]}>
        <span
          className={props.tone === "info" ? "text-ink-muted" : "text-alert"}
        >
          <Icon name={statusIcon(tone)} />
        </span>
        <span className="flex-1 status-sentence">{props.children}</span>
        {statusAction(props.action)}
      </div>
    );
  return (
    <>
      <div role="status" aria-atomic="true">
        {tone === "error" ? null : content}
      </div>
      <div role="alert" aria-atomic="true">
        {tone === "error" ? content : null}
      </div>
    </>
  );
};
