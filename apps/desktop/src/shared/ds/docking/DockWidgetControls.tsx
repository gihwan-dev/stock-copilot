import type { ReactElement } from "react";
import { dockPanels } from "../../lib";
import { Icon } from "../Icon";
import { useDockSession } from "./controller";
import { DockPopover, useCloseDockMenu } from "./DockPopover";
import { dockText, dockTitle } from "./messages";

function Catalog(): ReactElement | null {
  const session = useDockSession();
  const close = useCloseDockMenu();
  if (session?.model === undefined) return null;
  const opened = dockPanels.filter(
    (id) => session.model?.getNodeById(id) !== undefined,
  );
  const reset = (): void => {
    session.reset();
    close();
  };
  return (
    <>
      <ul data-dock-catalog="" className="flex flex-col">
        {dockPanels.map((panel) => (
          <li key={panel}>
            <button
              type="button"
              className="flex items-center justify-between gap-space-5 w-full px-space-2 py-space-1 rounded-sm cursor-pointer hover:bg-surface-sunken disabled:cursor-not-allowed disabled:text-ink-muted focus-visible:outline-focus"
              disabled={opened.includes(panel)}
              onClick={(): void => {
                close();
                session.show(panel);
              }}
            >
              <span>{dockTitle[panel]}</span>
              {opened.includes(panel) ? (
                <span className="text-caption text-ink-muted">
                  {dockText.open}
                </span>
              ) : (
                <Icon name="plus" />
              )}
            </button>
          </li>
        ))}
      </ul>
      <button
        type="button"
        data-dock-footer=""
        className="flex items-center gap-space-2 w-full py-space-1 px-space-2 border-t-hairline border-solid border-line cursor-pointer hover:text-primary focus-visible:outline-focus"
        onClick={reset}
      >
        <Icon name="refresh-cw" />
        {dockText.reset}
      </button>
    </>
  );
}
export function DockWidgetControls(): ReactElement {
  return (
    <DockPopover>
      <Catalog />
    </DockPopover>
  );
}
