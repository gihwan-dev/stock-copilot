import type {
  WorkspaceNode,
  TabsetNode,
  SplitNode,
} from "react-resizable-panels";
import {
  dockTab,
  type DockProfile,
  type DockModel,
  type DockPanel,
} from "./dock-panels";

export {
  dockPanels,
  dockTab,
  type DockProfile,
  type DockModel,
  type DockPanel,
} from "./dock-panels";
export { parseDockPanel, restoreDock } from "./brand";

const group = (
  first: DockPanel,
  others: readonly DockPanel[] = [],
): TabsetNode<DockPanel> => ({
  type: "tabset",
  id: `group-${first}`,
  selectedId: first,
  tabs: [first, ...others].map(dockTab),
});

type FrameSize = 25 | 32 | 35 | 36 | 40 | 42 | 48 | 52 | 58 | 60 | 64 | 68 | 75;
type Cluster = "portfolio" | "reading" | "monitor" | "dashboard";
const sized = (
  node: Readonly<WorkspaceNode<DockPanel>>,
  size: FrameSize,
): WorkspaceNode<DockPanel> => ({ ...node, size });
const split = (
  id: Cluster,
  orientation: "horizontal" | "vertical",
  children: readonly WorkspaceNode<DockPanel>[],
): SplitNode<DockPanel> => ({
  type: "split",
  id,
  orientation,
  children: [...children],
});

const makeRoot = (profile: DockProfile): WorkspaceNode<DockPanel> => {
  const overview = group("overview");
  const positions = group("positions");
  const briefing = group("briefing");
  const signals = group("signals");
  const allocation = group("allocation");
  const research = group("holdings", ["candidates", "economy"]);

  if (profile === "compact") {
    return split("dashboard", "vertical", [
      sized(group("overview", ["positions", "allocation", "signals"]), 48),
      sized(group("briefing", ["holdings", "candidates", "economy"]), 52),
    ]);
  }
  if (profile === "three-columns") {
    return split("dashboard", "horizontal", [
      sized(
        split("portfolio", "vertical", [
          sized(overview, 32),
          sized(positions, 68),
        ]),
        35,
      ),
      sized(
        split("reading", "vertical", [
          sized(briefing, 60),
          sized(research, 40),
        ]),
        40,
      ),
      sized(
        split("monitor", "vertical", [
          sized(signals, 25),
          sized(allocation, 75),
        ]),
        25,
      ),
    ]);
  }
  return split("dashboard", "horizontal", [
    sized(
      split("portfolio", "vertical", [
        sized(overview, profile === "summary-row" ? 58 : 36),
        sized(
          group("positions", ["allocation"]),
          profile === "summary-row" ? 42 : 64,
        ),
      ]),
      48,
    ),
    sized(
      split("reading", "vertical", [
        sized(group("briefing", ["signals"]), 60),
        sized(research, 40),
      ]),
      52,
    ),
  ]);
};

export const defaultDock = (profile: DockProfile): DockModel => ({
  version: 1,
  root: makeRoot(profile),
});
