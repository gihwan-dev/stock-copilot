import {
  createContext,
  use,
  useCallback,
  useState,
  type ReactElement,
} from "react";
import { Icon } from "../Icon";
import { Popover } from "../Popover";
import { dockText } from "./messages";

const CloseContext = createContext<() => void>(() => undefined);
export const useCloseDockMenu = (): (() => void) => use(CloseContext);
export function DockPopover(props: {
  readonly children: ReactElement;
}): ReactElement {
  const [visibility, setVisibility] = useState<"open" | "closed">("closed");
  const close = useCallback((): void => {
    setVisibility("closed");
  }, []);
  const trigger = (
    <button
      type="button"
      aria-label={dockText.add}
      data-dock-trigger="add"
      className="inline-flex flex-none items-center justify-center gap-space-1 px-space-2 py-space-1 rounded-sm text-label text-ink-muted cursor-pointer hover:bg-surface-sunken hover:text-primary focus-visible:outline-focus"
    >
      <Icon name="plus" />
      {dockText.add}
    </button>
  );
  return (
    <Popover
      label={dockText.add}
      trigger={trigger}
      visibility={visibility}
      onChange={setVisibility}
    >
      <CloseContext value={close}>
        <div data-dock-menu="add" className="flex flex-col gap-space-2">
          {props.children}
        </div>
      </CloseContext>
    </Popover>
  );
}
