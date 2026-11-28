import type { ReactElement } from "react";
import { defaultDock, dockPanels } from "../../lib";
import { Icon } from "../Icon";
import { groupsOf, useOpenDockWidget, useDockSession } from "./controller";
import { DockPopover, useCloseDockMenu } from "./DockPopover";
import { dockText, dockTitle } from "./messages";
import { labelDock } from "./labels";

function Catalog(): ReactElement | null {
  const session = useDockSession();
  const show = useOpenDockWidget();
  const close = useCloseDockMenu();
  if (session === undefined) return null;
  const model = session.handle?.getModel() ?? session.initial;
  if (model === undefined) return null;
  const opened = groupsOf(model).flatMap((entry) =>
    entry.tabs.map((tab) => tab.data),
  );
  const reset = (): void => {
    const defaults = defaultDock(session.profile);
    if (session.handle === null) session.update(defaults);
    else {
      const restored = session.handle.setModel(labelDock(defaults));
      session.update(restored);
      session.onFocusTab(groupsOf(restored).at(0)?.tabs.at(0)?.data);
    }
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
                show(panel);
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
    <DockPopover kind="add" label={dockText.add}>
      <Catalog />
    </DockPopover>
  );
}
