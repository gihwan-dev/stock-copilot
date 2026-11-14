import type { ReactElement } from "react";
import type { UiText } from "../lib";
type Tone = "neutral" | "primary" | "up" | "down" | "alert";
type Props = {
  readonly children: UiText;
  readonly tone?: Tone;
  readonly mono?: "normal" | "mono";
};
const styles = {
  neutral:
    "inline-flex items-center gap-space-1 px-space-2 rounded-sm text-size-sm leading-md font-medium whitespace-nowrap bg-surface-sunken text-ink-muted",
  primary:
    "inline-flex items-center gap-space-1 px-space-2 rounded-sm text-size-sm leading-md font-medium whitespace-nowrap bg-primary-soft text-primary",
  up: "inline-flex items-center gap-space-1 px-space-2 rounded-sm text-size-sm leading-md font-medium whitespace-nowrap bg-up-soft text-up",
  down: "inline-flex items-center gap-space-1 px-space-2 rounded-sm text-size-sm leading-md font-medium whitespace-nowrap bg-down-soft text-down",
  alert:
    "inline-flex items-center gap-space-1 px-space-2 rounded-sm text-size-sm leading-md font-medium whitespace-nowrap bg-alert-soft text-alert",
} as const;
export const Badge = (props: Props): ReactElement => {
  const tone = props.tone ?? "neutral";
  return (
    <span data-badge="" className={styles[tone]}>
      <span
        className={
          props.mono === "mono"
            ? "font-mono text-size-xs"
            : "font-sans text-size-sm"
        }
      >
        {props.children}
      </span>
    </span>
  );
};
