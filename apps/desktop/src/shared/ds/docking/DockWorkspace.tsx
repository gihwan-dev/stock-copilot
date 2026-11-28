import { useCallback, type ReactElement } from "react";
import {
  TabsetDragHandle,
  useParentTabsetState,
  Workspace,
  type Orientation,
  type TabNode,
  type WorkspaceClassNames,
  type WorkspaceImperativeHandle,
} from "react-resizable-panels";
import type { DockPanel } from "../../lib";
import { WidgetFrame } from "../WidgetFrame";
import { ScrollArea } from "../ScrollArea";
import { useWidgetAppearance } from "../AppFrame";
import { useDockSession, groupsOf, type DockGroup } from "./controller";
import { DockDivider } from "./DockDivider";
import { FrameMenu, activeDockPanel } from "./FrameMenu";
import { dockText, dockTitle } from "./messages";
import { removeStaticPanel } from "../../lib";
import { labelDock } from "./labels";

type Sections = {
  readonly overview: ReactElement;
  readonly positions: ReactElement;
  readonly briefing: ReactElement;
  readonly signals: ReactElement;
  readonly allocation: ReactElement;
  readonly holdings: ReactElement;
  readonly candidates: ReactElement;
  readonly economy: ReactElement;
};
const tabsetRadius = {
  "0": "flex flex-col bg-surface w-full h-full scroll-flex overflow-hidden",
  "4": "flex flex-col bg-surface w-full h-full scroll-flex overflow-hidden rounded-sm",
  "8": "flex flex-col bg-surface w-full h-full scroll-flex overflow-hidden rounded-md",
  "12": "flex flex-col bg-surface w-full h-full scroll-flex overflow-hidden rounded-lg",
} as const;
const workspaceClasses = {
  header:
    "flex flex-none items-center w-full min-h-control-height-sm bg-bg border-b-hairline border-solid border-line select-none cursor-grab active:cursor-grabbing",
  tabList: "flex items-center scroll-flex overflow-x-auto",
  tab: "flex-none whitespace-nowrap px-space-3 py-space-1 text-body-sm font-medium text-ink-muted cursor-grab hover:bg-surface-sunken aria-selected:bg-surface aria-selected:text-ink aria-selected:font-semibold border-b-hairline border-solid border-transparent aria-selected:border-primary focus-visible:outline-focus focus-visible:outline-focus-ring",
  toolbar: "flex flex-1 items-center justify-end self-stretch px-space-1",
  tabPanels: "flex flex-col flex-1 scroll-flex overflow-hidden",
  tabContent: "flex flex-col h-full w-full scroll-flex",
  floatingTabset:
    "max-w-full max-h-full bg-surface border-hairline border-solid border-line-strong shadow-popover",
  dropIndicator:
    "bg-primary opacity-area border-hairline border-solid border-primary",
  dragPreview:
    "rounded-sm bg-surface-raised text-primary px-space-2 py-space-1 shadow-popover text-body-sm",
} as const satisfies WorkspaceClassNames;
function FrameHeader({ panel }: { readonly panel: DockPanel }): ReactElement {
  const state = useParentTabsetState();
  return (
    <>
      {state?.isDragDisabled === false && (
        <TabsetDragHandle
          title={dockText.dragFrame}
          className="flex flex-1 self-stretch items-center justify-end px-space-2 text-ink-muted select-none"
        >
          {state.isFloating ? (
            <span aria-hidden="true">{dockText.grip}</span>
          ) : null}
        </TabsetDragHandle>
      )}
      <FrameMenu panel={panel} />
    </>
  );
}
const toolbar = (group: Readonly<DockGroup>): ReactElement | null => {
  const panel = activeDockPanel(group);
  return panel === undefined ? null : <FrameHeader panel={panel} />;
};
const separator = (axis: Orientation): ReactElement => (
  <DockDivider axis={axis} />
);
const empty = (): ReactElement => (
  <div
    data-dock-empty=""
    className="flex flex-1 items-center justify-center text-body-sm text-ink-muted"
  >
    {dockText.empty}
  </div>
);

function StaticWorkspace({
  sections,
}: {
  readonly sections: Sections;
}): ReactElement | null {
  const session = useDockSession();
  if (session?.initial === undefined) return null;
  const model = session.initial;
  const visible = groupsOf(model).flatMap((group) =>
    group.tabs.map((tab) => tab.data),
  );
  return (
    <div
      data-dock-workspace=""
      className="flex flex-col flex-1 scroll-flex w-full"
    >
      <ScrollArea direction="vertical">
        <div className="flex flex-col w-full">
          {visible.length === 0
            ? empty()
            : visible.map((panel) => (
                <section
                  key={panel}
                  data-tab-id={panel}
                  tabIndex={-1}
                  aria-label={dockTitle[panel]}
                >
                  <div className="flex justify-end px-space-2">
                    <button
                      type="button"
                      className="px-space-2 py-space-1 text-caption text-ink-muted cursor-pointer hover:text-primary"
                      onClick={(): void => {
                        session.update(removeStaticPanel(model, panel));
                      }}
                    >
                      {dockText.remove}
                    </button>
                  </div>
                  {sections[panel]}
                </section>
              ))}
        </div>
      </ScrollArea>
    </div>
  );
}
export function DockWorkspace({
  sections,
}: {
  readonly sections: Sections;
}): ReactElement | null {
  const session = useDockSession();
  const { radius } = useWidgetAppearance();
  const initial = session?.initial;
  const attach = session?.attach;
  const register = useCallback(
    (next: WorkspaceImperativeHandle<DockPanel> | null): void => {
      if (
        next !== null &&
        initial !== undefined &&
        next.getModel().root.id === "dashboard-initializing"
      ) {
        const restored = next.setModel(labelDock(initial));
        if (restored.root.id === "dashboard-initializing") return;
      }
      if (attach !== undefined) attach(next);
    },
    [attach, initial],
  );
  const render = useCallback(
    (tab: Readonly<TabNode<DockPanel>>): ReactElement => (
      <ScrollArea
        direction="both"
        dockPanel={tab.data}
        label={dockTitle[tab.data]}
      >
        <WidgetFrame panel={tab.data} title={dockTitle[tab.data]}>
          {sections[tab.data]}
        </WidgetFrame>
      </ScrollArea>
    ),
    [sections],
  );
  if (session?.initial === undefined) return null;
  if (session.mode === "static") return <StaticWorkspace sections={sections} />;
  return (
    <Workspace<DockPanel>
      key={session.profile}
      data-dock-workspace=""
      aria-label={dockText.workspace}
      defaultModel={{
        version: 1,
        root: { type: "tabset", id: "dashboard-initializing", tabs: [] },
      }}
      className="relative flex flex-col flex-1 w-full h-full scroll-flex bg-bg"
      classNames={{ ...workspaceClasses, tabset: tabsetRadius[radius] }}
      workspaceRef={register}
      onModelChanged={session.update}
      renderTabContent={render}
      renderSeparator={separator}
      renderTabsetToolbar={toolbar}
      renderEmptyTabset={empty}
    />
  );
}
