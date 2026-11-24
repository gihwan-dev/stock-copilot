import { createContext, use, type ReactElement } from "react";
import { pendingScreenSources, type ScreenSources } from "../shared/ipc";
import type { AppRoutePath } from "../shared/lib";

export type NavigateFn = (path: AppRoutePath) => void;

const NavigationContext = createContext<NavigateFn>((): void => undefined);

const SourcesContext = createContext<ScreenSources>(pendingScreenSources);

export const useNavigate = (): NavigateFn => use(NavigationContext);

export const useSources = (): ScreenSources => use(SourcesContext);

export const ScreenProviders = (props: {
  readonly go: NavigateFn;
  readonly sources: ScreenSources;
  readonly children: ReactElement;
}): ReactElement => (
  <NavigationContext value={props.go}>
    <SourcesContext value={props.sources}>{props.children}</SourcesContext>
  </NavigationContext>
);
