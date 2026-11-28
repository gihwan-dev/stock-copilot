import type { UiText } from "./brand";
import { groupsOf, addStaticPanel } from "./dock-frames";
import { createContext, use, useMemo, useState } from "react";
import {
  useWorkspaceCallbackRef,
  type WorkspaceImperativeHandle,
  type WorkspaceChangedMeta,
} from "react-resizable-panels";
import type { DockPanel, DockModel, DockProfile } from "./dock-panels";
type Sources = {
  readonly profile: DockProfile;
  readonly initial: DockModel | undefined;
  readonly mode: "resizable" | "static";
  readonly onFocusTab: (panel?: DockPanel) => void;
  readonly persist: (
    profile: DockProfile,
    model: DockModel,
    handle: Readonly<WorkspaceImperativeHandle<DockPanel>> | null,
  ) => void;
};
type Preferences = {
  readonly profile: DockProfile;
  readonly initial: DockModel | undefined;
  readonly handle: WorkspaceImperativeHandle<DockPanel> | null;
  readonly mode: Sources["mode"];
  readonly attach: (next: WorkspaceImperativeHandle<DockPanel> | null) => void;
  readonly onFocusTab: Sources["onFocusTab"];
  readonly update: (
    next: DockModel,
    change?: Readonly<WorkspaceChangedMeta>,
  ) => void;
};
const PreferencesContext = createContext<Preferences | undefined>(undefined);
export const dockPreferencesScope = {
  Context: PreferencesContext,
  useSession: (): Preferences | undefined => use(PreferencesContext),
} as const;
export function useDockPreferences({
  profile,
  initial,
  mode,
  onFocusTab,
  persist,
}: Sources): Preferences {
  const [handle, attach] = useWorkspaceCallbackRef<DockPanel>();
  const [edit, setEdit] = useState<{
    readonly profile: DockProfile;
    readonly model: DockModel;
  }>();
  return useMemo(
    (): Preferences => ({
      profile,
      initial:
        mode === "static" && edit?.profile === profile ? edit.model : initial,
      mode,
      handle,
      attach,
      onFocusTab,
      update(next, change): void {
        if (change?.reason === "set-model") return;
        if (mode === "static") setEdit({ profile, model: next });
        persist(profile, next, handle);
      },
    }),
    [profile, initial, mode, handle, attach, onFocusTab, persist, edit],
  );
}

const selectOrAdd = (
  handle: Readonly<WorkspaceImperativeHandle<DockPanel>>,
  panel: DockPanel,
  title: UiText,
): void => {
  const groups = groupsOf(handle.getModel());
  if (groups.some((group) => group.tabs.some((tab) => tab.data === panel))) {
    handle.selectTab(panel);
    return;
  }
  const target = handle.getActiveTabsetId() ?? groups.at(0)?.id;
  const added = handle.addTab(
    { id: panel, data: panel, title, disableClose: true },
    { tabsetId: target, select: true },
  );
  if (!added) handle.selectTab(panel);
};
export const useRevealDockWidget = (): ((
  panel: DockPanel,
  title: UiText,
) => void) => {
  const session = dockPreferencesScope.useSession();
  return (panel, title): void => {
    if (session === undefined) return;
    if (session.mode === "static" && session.initial !== undefined) {
      if (
        !groupsOf(session.initial).some((group) =>
          group.tabs.some((tab) => tab.data === panel),
        )
      )
        session.update(addStaticPanel(session.initial, panel));
    } else if (session.handle !== null)
      selectOrAdd(session.handle, panel, title);
    session.onFocusTab(panel);
  };
};
