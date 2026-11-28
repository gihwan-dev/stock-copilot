import type { TabsetNode, WorkspaceNode } from "react-resizable-panels";
import { dockTab, type DockModel, type DockPanel } from "./dock-panels";
type Frame = Readonly<WorkspaceNode<DockPanel>>;
const remove = (node: Readonly<Frame>, panel: DockPanel): Frame | undefined => {
  if (node.type === "tabset") {
    const tabs = node.tabs.filter((tab) => tab.data !== panel);
    const selected =
      tabs.find((tab) => tab.id === node.selectedId) ?? tabs.at(0);
    return selected === undefined
      ? undefined
      : { ...node, tabs, selectedId: selected.id };
  }
  const children = node.children.flatMap((child) => {
    const kept = remove(child, panel);
    return kept === undefined ? [] : [kept];
  });
  const first = children.at(0);
  if (children.length > 1) return { ...node, children };
  return first === undefined ? undefined : { ...first, size: node.size };
};
const append = (node: Readonly<Frame>, panel: DockPanel): Frame =>
  node.type === "tabset"
    ? { ...node, tabs: [...node.tabs, dockTab(panel)], selectedId: panel }
    : {
        ...node,
        children: node.children.map((child, index) =>
          index === 0 ? append(child, panel) : child,
        ),
      };
export const addStaticPanel = (
  model: DockModel,
  panel: DockPanel,
): DockModel => ({ ...model, root: append(model.root, panel) });
export const removeStaticPanel = (
  model: DockModel,
  panel: DockPanel,
): DockModel => ({
  version: 1,
  root: remove(model.root, panel) ?? {
    type: "tabset",
    id: "dashboard-empty",
    tabs: [],
  },
  floating: model.floating?.flatMap((frame) => {
    const updated = remove(frame, panel);
    return updated?.type === "tabset"
      ? [{ ...frame, tabs: updated.tabs, selectedId: updated.selectedId }]
      : [];
  }),
});

export type DockGroup = Readonly<TabsetNode<DockPanel>>;
const dockedGroups = (
  node: Readonly<WorkspaceNode<DockPanel>>,
): readonly DockGroup[] =>
  node.type === "tabset" ? [node] : node.children.flatMap(dockedGroups);
export const groupsOf = (model: Readonly<DockModel>): readonly DockGroup[] => [
  ...dockedGroups(model.root),
  ...(model.floating ?? []),
];
