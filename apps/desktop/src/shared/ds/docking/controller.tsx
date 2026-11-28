import {
  dockPreferencesScope,
  useRevealDockWidget,
  type DockPanel,
} from "../../lib";
import { dockTitle } from "./messages";
export { groupsOf, type DockGroup } from "../../lib";
export const useDockSession = dockPreferencesScope.useSession;
export const useOpenDockWidget = (): ((panel: DockPanel) => void) => {
  const reveal = useRevealDockWidget();
  return (panel) => {
    reveal(panel, dockTitle[panel]);
  };
};
