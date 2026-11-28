import type {
  IJsonModel,
  IJsonRowNode,
  IJsonSubLayout,
  IJsonTabSetNode,
} from "flexlayout-react";
import type { WorkspaceNode, TabsetNode } from "react-resizable-panels";
import type { DockModel, DockPanel } from "./dock-panels";

export type DockSnapshot =
  DockModel | { readonly version: 2; readonly layout: IJsonModel };

const legacyGroup = (
  group: Readonly<TabsetNode<DockPanel>>,
  model: DockModel,
): IJsonTabSetNode => ({
  type: "tabset",
  id: group.id,
  selected: Math.max(
    0,
    group.tabs.findIndex((tab) => tab.id === group.selectedId),
  ),
  ...(group.id === model.maximizedTabsetId ? { maximized: true } : {}),
  children: group.tabs.map(({ data }) => ({
    type: "tab",
    id: data,
    name: data,
    component: data,
  })),
});

const legacyNode = (
  node: Readonly<WorkspaceNode<DockPanel>>,
  model: DockModel,
): IJsonRowNode | IJsonTabSetNode => ({
  ...(node.type === "tabset"
    ? legacyGroup(node, model)
    : {
        type: "row",
        id: node.id,
        children: node.children.map((child) => legacyNode(child, model)),
      }),
  ...(typeof node.size === "number" ? { weight: node.size } : {}),
});
const migrate = (model: DockModel): IJsonModel => ({
  global: {
    rootOrientationVertical:
      model.root.type === "split" && model.root.orientation === "vertical",
  },
  layout:
    model.root.type === "split"
      ? {
          id: model.root.id,
          type: "row",
          children: model.root.children.map((node) => legacyNode(node, model)),
        }
      : {
          type: "row",
          children: [legacyGroup(model.root, model)],
        },
  subLayouts: Object.fromEntries(
    (model.floating ?? []).map((frame) => [
      `float-${frame.id}`,
      {
        type: "float",
        rect: {
          x: frame.x,
          y: frame.y,
          width: frame.width,
          height: frame.height,
        },
        layout: {
          type: "row",
          children: [legacyGroup(frame, model)],
        },
      } satisfies IJsonSubLayout,
    ]),
  ),
});
export const widgetLayout = (snapshot: DockSnapshot): IJsonModel => {
  const layout = snapshot.version === 1 ? migrate(snapshot) : snapshot.layout;
  return {
    ...layout,
    global: {
      ...layout.global,
      tabEnableFloat: true,
      tabEnableFloatIcon: true,
      tabEnablePopout: false,
      tabEnableRename: false,
      tabEnablePin: true,
      tabSetEnableTabGroups: true,
      tabSetEnableMaximize: true,
    },
  };
};
