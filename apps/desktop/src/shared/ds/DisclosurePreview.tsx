import { useId, type ReactElement } from "react";
import type { FreeText, UiText } from "../lib";
import { DataText } from "./Numbers";
import { Icon } from "./Icon";

export type PreviewProps = {
  readonly preview: ReactElement;
  readonly children: ReactElement;
  readonly title: UiText;
  readonly closeTitle: UiText;
  readonly detail: FreeText | undefined;
  readonly open: "open" | "closed";
  readonly onOpenChange: (value: "open" | "closed") => void;
};
export const DisclosurePreview = ({
  config: props,
}: {
  readonly config: PreviewProps;
}): ReactElement => {
  const id = useId();
  const next = props.open === "open" ? "closed" : "open";
  return (
    <div>
      <div id={id}>
        {props.open === "open" ? props.children : props.preview}
      </div>
      <button
        type="button"
        aria-controls={id}
        aria-expanded={props.open === "open"}
        onClick={(): void => {
          props.onOpenChange(next);
        }}
        className="inline-flex items-center gap-space-1 rounded-md py-space-2 mt-space-1 px-space-1 text-ink text-body font-semibold cursor-pointer hover:bg-surface-sunken focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:outline-offset-focus"
      >
        <Icon name={props.open === "open" ? "chevron-up" : "chevron-down"} />
        <span>{props.open === "open" ? props.closeTitle : props.title}</span>
        {props.open === "open" || props.detail === undefined ? null : (
          <DataText value={props.detail} variant="caption" />
        )}
      </button>
    </div>
  );
};
