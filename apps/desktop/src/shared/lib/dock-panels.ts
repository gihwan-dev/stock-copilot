import type { TabNode, WorkspaceModel } from "react-resizable-panels";

export const dockPanels = [
  "overview",
  "positions",
  "briefing",
  "signals",
  "allocation",
  "holdings",
  "candidates",
  "economy",
] as const;

export type DockPanel = NonNullable<ReturnType<typeof dockPanels.at>>;
export type DockProfile =
  "compact" | "summary-row" | "two-columns" | "three-columns";
export type DockModel = Readonly<WorkspaceModel<DockPanel>>;

export const dockTab = (panel: DockPanel): TabNode<DockPanel> => ({
  id: panel,
  data: panel,
});
