import { createContext, use, useMemo, useState } from "react";
import {
  Actions,
  DockLocation,
  Model,
  type Action,
  type ILayoutProps,
} from "flexlayout-react";
import { defaultDock } from "./dock";
import { widgetLayout, type DockSnapshot } from "./widget-layout";
import { dockPanels, type DockPanel, type DockProfile } from "./dock-panels";

export type DockSources = {
  readonly profile: DockProfile;
  readonly initial: DockSnapshot | undefined;
  readonly mode: "resizable" | "static";
  readonly onFocusTab: (panel?: DockPanel) => void;
  readonly persist: (
    profile: DockProfile,
    model: Extract<DockSnapshot, { version: 2 }>,
  ) => void;
};
type Preferences = DockSources & {
  readonly model: Model | undefined;
  readonly menu: NonNullable<ILayoutProps["onContextMenu"]>;
  readonly save: () => void;
  readonly reset: () => void;
  readonly show: (panel: DockPanel) => void;
  readonly close: (panel: DockPanel) => void;
};
const PreferencesContext = createContext<Preferences | undefined>(undefined);
export const dockPreferencesScope = {
  Context: PreferencesContext,
  useSession: (): Preferences | undefined => use(PreferencesContext),
} as const;

export function useDockPreferences(
  options: DockSources,
  menu: Preferences["menu"],
  dispatch: (model: Readonly<Model>, action: Readonly<Action>) => void,
): Preferences {
  const { profile, mode, initial, persist, onFocusTab } = options;
  const [edit, setEdit] = useState<DockSnapshot>();
  const snapshot = edit ?? initial;
  const model = useMemo(
    () =>
      snapshot === undefined
        ? undefined
        : Model.fromJson(widgetLayout(snapshot)),
    [snapshot],
  );
  return {
    ...options,
    model,
    menu,
    save(): void {
      if (model === undefined) return;
      const next = { version: 2, layout: model.toJson() } as const;
      persist(profile, next);
      if (mode === "static") setEdit(next);
    },
    reset(): void {
      const defaults = {
        version: 2,
        layout: Model.fromJson(widgetLayout(defaultDock(profile))).toJson(),
      } as const;
      setEdit(defaults);
      persist(profile, defaults);
      onFocusTab(dockPanels[0]);
    },
    show(panel): void {
      if (model === undefined) return;
      if (model.getNodeById(panel) === undefined) {
        const target =
          model.getActiveTabset() ??
          model.getFirstTabSet() ??
          model.getRootRow();
        if (target !== undefined)
          dispatch(
            model,
            Actions.addTab(
              { type: "tab", id: panel, component: panel, name: panel },
              target.getId(),
              DockLocation.CENTER,
              -1,
              true,
            ),
          );
      } else dispatch(model, Actions.selectTab(panel));
      onFocusTab(panel);
    },
    close(panel): void {
      if (model !== undefined) dispatch(model, Actions.deleteTab(panel));
    },
  };
}
export const useRevealDockWidget = (): ((panel: DockPanel) => void) => {
  const session = dockPreferencesScope.useSession();
  return (panel): void => {
    if (session !== undefined) session.show(panel);
  };
};
