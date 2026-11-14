import { useState, type ReactElement } from "react";
import { Collapsible } from "@base-ui/react/collapsible";
import type { UiText } from "../lib";
import { Icon } from "./Icon";
type State = "open" | "closed";
type Props = {
  readonly title: UiText;
  readonly children: ReactElement;
  readonly open?: State;
  readonly defaultOpen?: State;
  readonly onOpenChange?: (state: State) => void;
};
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
      <Collapsible.Trigger className="flex items-center gap-space-2 w-full h-toolbar-height bg-transparent text-ink text-body font-semibold cursor-pointer focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:outline-offset-focus">
        <Icon name={state === "open" ? "chevron-down" : "chevron-right"} />
        {props.title}
      </Collapsible.Trigger>
      <Collapsible.Panel className="pb-space-4 flex flex-col gap-space-4">
        {props.children}
      </Collapsible.Panel>
    </Collapsible.Root>
  );
};
