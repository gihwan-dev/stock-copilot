import type { ReactElement } from "react";
import type { UiText } from "../lib";
import type { ColorToken, TypeStyleToken } from "./tokens";
type Props = {
  readonly children: UiText;
  readonly variant?: Extract<
    TypeStyleToken,
    "body" | "body-sm" | "label" | "caption" | "code"
  >;
  readonly tone?: Extract<
    ColorToken,
    "ink" | "ink-muted" | "ink-subtle" | "primary" | "alert"
  >;
  readonly as?: "p" | "span" | "div";
};
const normal = (props: Props): ReactElement => {
  const Tag = props.as ?? "p";
  if (props.variant === "body-sm")
    return <Tag className="text-body-sm text-ink">{props.children}</Tag>;
  if (props.variant === "label")
    return <Tag className="text-label text-ink">{props.children}</Tag>;
  if (props.variant === "caption")
    return <Tag className="text-caption text-ink">{props.children}</Tag>;
  if (props.variant === "code")
    return <Tag className="text-code text-ink">{props.children}</Tag>;
  return <Tag className="text-body text-ink">{props.children}</Tag>;
};
const muted = (props: Props): ReactElement => {
  const Tag = props.as ?? "p";
  if (props.variant === "body-sm")
    return <Tag className="text-body-sm text-ink-muted">{props.children}</Tag>;
  if (props.variant === "label")
    return <Tag className="text-label text-ink-muted">{props.children}</Tag>;
  if (props.variant === "caption")
    return <Tag className="text-caption text-ink-muted">{props.children}</Tag>;
  if (props.variant === "code")
    return <Tag className="text-code text-ink-muted">{props.children}</Tag>;
  return <Tag className="text-body text-ink-muted">{props.children}</Tag>;
};
const subtle = (props: Props): ReactElement => {
  const Tag = props.as ?? "p";
  if (props.variant === "body-sm")
    return <Tag className="text-body-sm text-ink-subtle">{props.children}</Tag>;
  if (props.variant === "label")
    return <Tag className="text-label text-ink-subtle">{props.children}</Tag>;
  if (props.variant === "caption")
    return <Tag className="text-caption text-ink-subtle">{props.children}</Tag>;
  if (props.variant === "code")
    return <Tag className="text-code text-ink-subtle">{props.children}</Tag>;
  return <Tag className="text-body text-ink-subtle">{props.children}</Tag>;
};
const primary = (props: Props): ReactElement => {
  const Tag = props.as ?? "p";
  if (props.variant === "body-sm")
    return <Tag className="text-body-sm text-primary">{props.children}</Tag>;
  if (props.variant === "label")
    return <Tag className="text-label text-primary">{props.children}</Tag>;
  if (props.variant === "caption")
    return <Tag className="text-caption text-primary">{props.children}</Tag>;
  if (props.variant === "code")
    return <Tag className="text-code text-primary">{props.children}</Tag>;
  return <Tag className="text-body text-primary">{props.children}</Tag>;
};
const alert = (props: Props): ReactElement => {
  const Tag = props.as ?? "p";
  if (props.variant === "body-sm")
    return <Tag className="text-body-sm text-alert">{props.children}</Tag>;
  if (props.variant === "label")
    return <Tag className="text-label text-alert">{props.children}</Tag>;
  if (props.variant === "caption")
    return <Tag className="text-caption text-alert">{props.children}</Tag>;
  if (props.variant === "code")
    return <Tag className="text-code text-alert">{props.children}</Tag>;
  return <Tag className="text-body text-alert">{props.children}</Tag>;
};
export const Text = (props: Props): ReactElement => {
  if (props.tone === "ink-muted") return muted(props);
  if (props.tone === "ink-subtle") return subtle(props);
  if (props.tone === "primary") return primary(props);
  if (props.tone === "alert") return alert(props);
  if (props.tone === undefined && props.variant === "caption")
    return subtle(props);
  return normal(props);
};
