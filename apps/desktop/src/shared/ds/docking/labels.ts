import type { TabNode, WorkspaceNode } from "react-resizable-panels";
import type { DockModel, DockPanel } from "../../lib";
import { dockTitle } from "./messages";

export const labeledTab = (panel: DockPanel): TabNode<DockPanel> => ({
  id: panel,
  data: panel,
  title: dockTitle[panel],
  disableClose: true,
});
const labelNode = (
  node: Readonly<WorkspaceNode<DockPanel>>,
): WorkspaceNode<DockPanel> =>
  node.type === "split"
    ? { ...node, children: node.children.map(labelNode) }
    : { ...node, tabs: node.tabs.map((tab) => labeledTab(tab.data)) };
export const labelDock = (model: DockModel): DockModel => ({
  ...model,
  root: labelNode(model.root),
  floating: model.floating?.map((group) => ({
    ...group,
    tabs: group.tabs.map((tab) => labeledTab(tab.data)),
  })),
});
