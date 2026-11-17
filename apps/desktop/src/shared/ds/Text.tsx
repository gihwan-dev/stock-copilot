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
  ink: "status-sentence text-body text-ink",
  "ink-muted": "status-sentence text-body text-ink-muted",
  "ink-subtle": "status-sentence text-body text-ink-subtle",
  primary: "status-sentence text-body text-primary",
  alert: "status-sentence text-body text-alert",
} as const;
const classes1 = {
  ink: "status-sentence text-body-sm text-ink",
  "ink-muted": "status-sentence text-body-sm text-ink-muted",
  "ink-subtle": "status-sentence text-body-sm text-ink-subtle",
  primary: "status-sentence text-body-sm text-primary",
  alert: "status-sentence text-body-sm text-alert",
} as const;
const classes2 = {
  ink: "status-sentence text-label text-ink",
  "ink-muted": "status-sentence text-label text-ink-muted",
  "ink-subtle": "status-sentence text-label text-ink-subtle",
  primary: "status-sentence text-label text-primary",
  alert: "status-sentence text-label text-alert",
} as const;
const classes3 = {
  ink: "status-sentence text-caption text-ink",
  "ink-muted": "status-sentence text-caption text-ink-muted",
  "ink-subtle": "status-sentence text-caption text-ink-subtle",
  primary: "status-sentence text-caption text-primary",
  alert: "status-sentence text-caption text-alert",
} as const;
const classes4 = {
  ink: "status-sentence text-code text-ink",
  "ink-muted": "status-sentence text-code text-ink-muted",
  "ink-subtle": "status-sentence text-code text-ink-subtle",
  primary: "status-sentence text-code text-primary",
  alert: "status-sentence text-code text-alert",
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
