import {
  createContext,
  use,
  useCallback,
  useState,
  type ReactElement,
} from "react";
import type { UiText } from "../../lib";
import { Icon } from "../Icon";
import { Popover } from "../Popover";
import { dockText } from "./messages";

const CloseContext = createContext<() => void>(() => undefined);
export const useCloseDockMenu = (): (() => void) => use(CloseContext);
export function DockPopover(props: {
  readonly kind: "layout" | "add";
  readonly label: UiText;
  readonly children: ReactElement;
}): ReactElement {
  const [visibility, setVisibility] = useState<"open" | "closed">("closed");
  const close = useCallback((): void => {
    setVisibility("closed");
  }, []);
  const trigger = (
    <button
      type="button"
      aria-label={props.label}
      data-dock-trigger={props.kind}
      className="inline-flex flex-none items-center justify-center gap-space-1 px-space-2 py-space-1 rounded-sm text-label text-ink-muted cursor-pointer hover:bg-surface-sunken hover:text-primary focus-visible:outline-focus"
    >
      {props.kind === "add" ? <Icon name="plus" /> : null}
      {dockText[props.kind]}
      {props.kind === "layout" ? <Icon name="chevron-down" /> : null}
    </button>
  );
  return (
    <Popover
      label={props.label}
      trigger={trigger}
      visibility={visibility}
      onChange={setVisibility}
    >
      <CloseContext value={close}>
        <div data-dock-menu={props.kind} className="flex flex-col gap-space-2">
          {props.children}
        </div>
      </CloseContext>
    </Popover>
  );
}
