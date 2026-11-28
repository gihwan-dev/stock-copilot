import { createContext, use, type ReactElement } from "react";
import { Layout, type ILayoutProps, type TabNode } from "flexlayout-react";
import { dockPanels, parseDockPanel } from "../../lib";
import { WidgetFrame } from "../WidgetFrame";
import { ScrollArea } from "../ScrollArea";
import { useDockSession } from "./controller";
import { dockText, dockTitle, dockLabels } from "./messages";
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

const SectionContext = createContext<Sections | undefined>(undefined);
const Empty = (): ReactElement => (
  <div
    data-dock-empty=""
    className="flex flex-1 items-center justify-center text-body-sm text-ink-muted"
  >
    {dockText.empty}
  </div>
);
function Widget({
  panel,
}: {
  readonly panel: keyof Sections;
}): ReactElement | null {
  const sections = use(SectionContext);
  if (sections === undefined) return null;
  return (
    <div className="flex flex-col h-full w-full scroll-flex">
      <ScrollArea direction="both" dockPanel={panel} label={dockTitle[panel]}>
        <WidgetFrame panel={panel} title={dockTitle[panel]}>
          {sections[panel]}
        </WidgetFrame>
      </ScrollArea>
    </div>
  );
}
const factory = (node: Readonly<TabNode>): ReactElement | null => {
  const panel = parseDockPanel(node.getId());
  return panel === undefined ? null : <Widget panel={panel} />;
};
const labelByKey = new Map(Object.entries({ ...dockLabels, ...dockTitle }));
const translateDock: NonNullable<ILayoutProps["i18nTranslator"]> = (key) =>
  labelByKey.get(key) ?? key;
function StaticWorkspace({
  sections,
}: {
  readonly sections: Sections;
}): ReactElement | null {
  const session = useDockSession();
  if (session?.model === undefined) return null;
  const visible = dockPanels.filter(
    (panel) => session.model?.getNodeById(panel) !== undefined,
  );
  return (
    <ScrollArea direction="vertical">
      <div className="flex flex-col w-full">
        {visible.length === 0 ? (
          <Empty />
        ) : (
          visible.map((panel) => (
            <section
              key={panel}
              data-tab-id={panel}
              tabIndex={-1}
              aria-label={dockTitle[panel]}
            >
              <button
                type="button"
                onClick={(): void => {
                  session.close(panel);
                }}
                className="px-space-2 py-space-1 text-caption text-ink-muted cursor-pointer hover:text-primary"
              >
                {dockText.remove}
              </button>
              {sections[panel]}
            </section>
          ))
        )}
      </div>
    </ScrollArea>
  );
}
export function DockWorkspace({
  sections,
}: {
  readonly sections: Sections;
}): ReactElement | null {
  const session = useDockSession();
  if (session?.model === undefined) return null;
  return (
    <div
      data-dock-workspace=""
      aria-label={dockText.workspace}
      className="relative flex flex-1 h-full w-full scroll-flex bg-bg"
    >
      {session.mode === "static" ? (
        <StaticWorkspace sections={sections} />
      ) : (
        <SectionContext value={sections}>
          <Layout
            model={session.model}
            factory={factory}
            realtimeResize={false}
            supportsPopout={false}
            constrainFloatPanels
            invalidateTabContentOnParentRender={false}
            i18nTranslator={translateDock}
            onContextMenu={session.menu}
            onTabSetPlaceHolder={Empty}
          />
        </SectionContext>
      )}
    </div>
  );
}
