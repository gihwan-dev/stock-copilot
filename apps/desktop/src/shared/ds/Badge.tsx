import type { ReactElement } from "react";
import type { UiText } from "../lib";
type Tone = "neutral" | "primary" | "alert";
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
  alert:
    "inline-flex items-center gap-space-1 px-space-2 rounded-sm text-size-sm leading-md font-medium whitespace-nowrap bg-alert-soft text-alert",
} as const;
const fonts = {
  normal: "font-sans text-size-sm",
  mono: "font-mono text-size-xs",
} as const;
export const Badge = (props: Props): ReactElement => {
  const tone = props.tone ?? "neutral";
  const mono = props.mono ?? "normal";
  return (
    <span data-badge="" className={styles[tone]}>
      <span className={fonts[mono]}>{props.children}</span>
    </span>
  );
};
