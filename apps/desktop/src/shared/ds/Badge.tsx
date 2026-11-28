import type { ReactElement } from "react";
import type { UiText } from "../lib";
type Tone = "neutral" | "primary" | "alert";
type Props = {
  readonly children: UiText;
  readonly tone?: Tone;
  readonly outline?: "visible";
  readonly mono?: "normal" | "mono";
};
const styles = {
  neutral:
    "inline-flex items-center gap-space-1 px-space-2 rounded-sm text-size-sm leading-md font-medium status-sentence max-w-full bg-surface-sunken text-ink-muted",
  primary:
    "inline-flex items-center gap-space-1 px-space-2 rounded-sm text-size-sm leading-md font-medium status-sentence max-w-full bg-primary-soft text-primary",
  alert:
    "inline-flex items-center gap-space-1 px-space-2 rounded-sm text-size-sm leading-md font-medium status-sentence max-w-full bg-alert-soft text-alert",
} as const;
const fonts = {
  normal: "font-sans text-size-sm",
  mono: "font-mono text-size-xs",
} as const;
export const Badge = (props: Props): ReactElement => {
  const tone = props.tone ?? "neutral";
  const mono = props.mono ?? "normal";
  if (props.outline === "visible" && tone === "alert")
    return (
      <span
        data-badge=""
        className="inline-flex items-center rounded-sm px-space-2 border-hairline border-solid border-alert bg-alert-soft text-alert"
      >
        <span className={fonts[mono]}>{props.children}</span>
      </span>
    );
  return (
    <span data-badge="" className={styles[tone]}>
      <span className={fonts[mono]}>{props.children}</span>
    </span>
  );
};
