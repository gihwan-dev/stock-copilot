import { useState, type ReactElement } from "react";
import { Collapsible } from "@base-ui/react/collapsible";
import type { FreeText, UiText } from "../lib";
import { DataText } from "./Numbers";
import { DisclosurePreview, type PreviewProps } from "./DisclosurePreview";
import { Icon } from "./Icon";
type State = "open" | "closed";
type Props = {
  readonly title: UiText;
  readonly detail?: FreeText | undefined;
  readonly children: ReactElement;
  readonly open?: State;
  readonly defaultOpen?: State;
  readonly onOpenChange?: (state: State) => void;
};
const CollapsibleDisclosure = ({
  config: props,
}: {
  readonly config: Props;
}): ReactElement => {
  const [inner, setInner] = useState<State>(props.defaultOpen ?? "closed");
  const state = props.open ?? inner;
  return (
    <Collapsible.Root
      open={state === "open"}
      onOpenChange={(next): void => {
        const value = next ? "open" : "closed";
        setInner(value);
        if (props.onOpenChange !== undefined) props.onOpenChange(value);
      }}
      className="border-t-hairline border-solid border-line"
    >
      <Collapsible.Trigger
        className={
          props.detail === undefined
            ? "flex items-center gap-space-2 rounded-md w-full min-h-toolbar-height py-space-2 text-left bg-transparent text-ink text-body font-semibold cursor-pointer focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:outline-offset-focus"
            : "inline-flex items-center gap-space-1 rounded-md min-h-control-height my-space-2 px-space-1 text-left bg-transparent text-ink text-body font-semibold cursor-pointer hover:bg-surface-sunken focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:outline-offset-focus"
        }
      >
        <span
          className={
            state === "open"
              ? "inline-flex transition-disclosure motion-reduce:transition-none rotate-disclosure"
              : "inline-flex transition-disclosure motion-reduce:transition-none"
          }
        >
          <Icon name="chevron-right" />
        </span>
        <span className="status-sentence">{props.title}</span>
        {props.detail === undefined ? null : (
          <DataText value={props.detail} variant="caption" />
        )}
      </Collapsible.Trigger>
      <Collapsible.Panel
        className={state === "closed" ? "hidden" : "flex flex-col"}
      >
        <div
          className={
            props.detail === undefined
              ? "pb-space-4 flex flex-col gap-space-4"
              : "pb-space-1 flex flex-col gap-space-2"
          }
        >
          {props.children}
        </div>
      </Collapsible.Panel>
    </Collapsible.Root>
  );
};

export const Disclosure = (props: Props | PreviewProps): ReactElement =>
  "preview" in props ? (
    <DisclosurePreview config={props} />
  ) : (
    <CollapsibleDisclosure config={props} />
  );
