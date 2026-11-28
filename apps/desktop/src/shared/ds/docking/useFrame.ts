import type { WorkspaceImperativeHandle } from "react-resizable-panels";
import type { DockModel, DockPanel } from "../../lib";
import { groupsOf, useDockSession, type DockGroup } from "./controller";
export type CurrentFrame = {
  readonly handle: WorkspaceImperativeHandle<DockPanel>;
  readonly focus: (panel?: DockPanel) => void;
  readonly model: DockModel;
  readonly group: DockGroup;
  readonly location: "floating" | "docked";
};
export function useFrame(panel: DockPanel): CurrentFrame | undefined {
  const session = useDockSession();
  if (session === undefined || session.handle === null) return undefined;
  const handle = session.handle;
  const model = handle.getModel();
  const group = groupsOf(model).find((item) =>
    item.tabs.some((tab) => tab.data === panel),
  );
  if (group === undefined) return undefined;
  const floating =
    model.floating?.some((item) => item.id === group.id) === true;
  return {
    focus: session.onFocusTab,
    handle,
    model,
    group,
    location: floating ? "floating" : "docked",
  };
}
