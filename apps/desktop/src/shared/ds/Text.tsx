import type { ReactElement } from "react";
import type { UiText } from "../lib";
import type { TypeStyleToken } from "./tokens";
type Props = {
  readonly children: UiText;
  readonly variant?: Extract<
    TypeStyleToken,
    "body" | "body-sm" | "label" | "caption" | "code"
  >;
  readonly tone?: "ink" | "ink-muted" | "ink-subtle" | "primary" | "alert";
  readonly as?: "p" | "span" | "div";
};
const classes0 = {
  ink: "text-body text-ink",
  "ink-muted": "text-body text-ink-muted",
  "ink-subtle": "text-body text-ink-subtle",
  primary: "text-body text-primary",
  alert: "text-body text-alert",
} as const;
const classes1 = {
  ink: "text-body-sm text-ink",
  "ink-muted": "text-body-sm text-ink-muted",
  "ink-subtle": "text-body-sm text-ink-subtle",
  primary: "text-body-sm text-primary",
  alert: "text-body-sm text-alert",
} as const;
const classes2 = {
  ink: "text-label text-ink",
  "ink-muted": "text-label text-ink-muted",
  "ink-subtle": "text-label text-ink-subtle",
  primary: "text-label text-primary",
  alert: "text-label text-alert",
} as const;
const classes3 = {
  ink: "text-caption text-ink",
  "ink-muted": "text-caption text-ink-muted",
  "ink-subtle": "text-caption text-ink-subtle",
  primary: "text-caption text-primary",
  alert: "text-caption text-alert",
} as const;
const classes4 = {
  ink: "text-code text-ink",
  "ink-muted": "text-code text-ink-muted",
  "ink-subtle": "text-code text-ink-subtle",
  primary: "text-code text-primary",
  alert: "text-code text-alert",
} as const;
export const Text = (props: Props): ReactElement => {
  const Tag = props.as ?? "p";
  const tone =
    props.tone ?? (props.variant === "caption" ? "ink-subtle" : "ink");
  if (props.variant === "body-sm")
    return <Tag className={classes1[tone]}>{props.children}</Tag>;
  if (props.variant === "label")
    return <Tag className={classes2[tone]}>{props.children}</Tag>;
  if (props.variant === "caption")
    return <Tag className={classes3[tone]}>{props.children}</Tag>;
  if (props.variant === "code")
    return <Tag className={classes4[tone]}>{props.children}</Tag>;
  return <Tag className={classes0[tone]}>{props.children}</Tag>;
};
