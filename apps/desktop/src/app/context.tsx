import { createContext, use, type ReactElement } from "react";
import { pendingScreenSources, type ScreenSources } from "../shared/ipc";
import type { AppRoutePath, EntityId } from "../shared/lib";

const InstrumentSelectionContext = createContext<
  (instrument: EntityId) => void
>((): void => undefined);
export const useInstrumentSelection = (): ((instrument: EntityId) => void) =>
  use(InstrumentSelectionContext);
export const InstrumentSelection = (props: {
  readonly value: (instrument: EntityId) => void;
  readonly children: ReactElement;
}): ReactElement => (
  <InstrumentSelectionContext value={props.value}>
    {props.children}
  </InstrumentSelectionContext>
);

export type NavigateFn = (path: AppRoutePath) => void;

export type SystemSettingsPresence = "shown" | "hidden";

const NavigationContext = createContext<NavigateFn>((): void => undefined);

const SettingsOriginContext = createContext<AppRoutePath>("/");
export const useSettingsOrigin = (): AppRoutePath => use(SettingsOriginContext);

const SourcesContext = createContext<ScreenSources>(pendingScreenSources);

const SystemSettingsContext = createContext<SystemSettingsPresence>("hidden");

export const useNavigate = (): NavigateFn => use(NavigationContext);

export const useSources = (): ScreenSources => use(SourcesContext);

export const useSystemSettings = (): SystemSettingsPresence =>
  use(SystemSettingsContext);

export const ScreenProviders = (props: {
  readonly origin: AppRoutePath;
  readonly go: NavigateFn;
  readonly sources: ScreenSources;
  readonly system: SystemSettingsPresence;
  readonly children: ReactElement;
}): ReactElement => (
  <NavigationContext value={props.go}>
    <SourcesContext value={props.sources}>
      <SystemSettingsContext value={props.system}>
        <SettingsOriginContext value={props.origin}>
          {props.children}
        </SettingsOriginContext>
      </SystemSettingsContext>
    </SourcesContext>
  </NavigationContext>
);
