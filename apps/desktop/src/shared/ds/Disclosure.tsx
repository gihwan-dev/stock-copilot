import { useState, type ReactElement } from "react";
import { Collapsible } from "@base-ui/react/collapsible";
import type { FreeText, UiText } from "../lib";
import { DataText } from "./Numbers";
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
const states = {
  open: "inline-flex transition-disclosure motion-reduce:transition-none rotate-disclosure",
  closed: "inline-flex transition-disclosure motion-reduce:transition-none",
} as const;
export const Disclosure = (props: Props): ReactElement => {
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
      <Collapsible.Trigger className="flex items-center gap-space-2 w-full min-h-toolbar-height py-space-2 text-left bg-transparent text-ink text-body font-semibold cursor-pointer focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:outline-offset-focus">
        <span className={states[state]}>
          <Icon name="chevron-right" />
        </span>
        <span className="status-sentence">{props.title}</span>
        {props.detail === undefined ? null : (
          <DataText value={props.detail} variant="caption" />
        )}
      </Collapsible.Trigger>
      <Collapsible.Panel
        className={
          state === "closed" ? "hidden" : "pb-space-4 flex flex-col gap-space-4"
        }
      >
        {props.children}
      </Collapsible.Panel>
    </Collapsible.Root>
  );
};
