import { useId, useState, type ReactElement } from "react";
import {
  parseEntityId,
  type EntityId,
  type FreeText,
  type UiText,
} from "../lib";
import { DataText } from "./Numbers";
import { Icon } from "./Icon";

export type PreviewProps = {
  readonly preview: ReactElement;
  readonly children: ReactElement;
  readonly title: UiText;
  readonly closeTitle: UiText;
  readonly detail: FreeText | undefined;
  readonly open: "open" | "closed";
  readonly onOpenChange: (
    value: "open" | "closed",
    trigger: EntityId,
    input: "keyboard" | "pointer",
  ) => void;
};
export const DisclosurePreview = ({
  config: props,
}: {
  readonly config: PreviewProps;
}): ReactElement | null => {
  const id = useId();
  const [input, setInput] = useState<"keyboard" | "pointer">("keyboard");
  const trigger = parseEntityId(`${id}-toggle`);
  if (trigger.kind === "err") return null;
  const next = props.open === "open" ? "closed" : "open";
  return (
    <div>
      <div id={id}>
        {props.open === "open" ? props.children : props.preview}
      </div>
      <button
        id={trigger.value}
        type="button"
        aria-controls={id}
        aria-expanded={props.open === "open"}
        onKeyDown={(): void => {
          setInput("keyboard");
        }}
        onPointerDown={(): void => {
          setInput("pointer");
        }}
        onClick={(): void => {
          props.onOpenChange(next, trigger.value, input);
        }}
        className="inline-flex items-center gap-space-1 rounded-md py-space-2 mt-space-1 px-space-1 text-ink text-body font-semibold cursor-pointer hover:bg-surface-sunken focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:outline-offset-focus"
      >
        <Icon name={props.open === "open" ? "chevron-down" : "chevron-right"} />
        <span>{props.open === "open" ? props.closeTitle : props.title}</span>
        {props.open === "open" || props.detail === undefined ? null : (
          <DataText value={props.detail} variant="caption" />
        )}
      </button>
    </div>
  );
};
