import { QueryClientProvider, type QueryClient } from "@tanstack/react-query";
import { useState, type ReactElement } from "react";
import { ErrorBoundary, type FallbackProps } from "react-error-boundary";
import { FirstRunGate, OnboardingClientProvider } from "../features/onboarding";
import { EmptyState, Page } from "../shared/ds";
import {
  getBriefings,
  pendingScreenSources,
  type ScreenSources,
  type SettingsClient,
} from "../shared/ipc";
import { text, type AppRoutePath } from "../shared/lib";
import { useCoreEvent } from "../shared/platform";
import { createAppQueryClient } from "./query";
import { AppRouter } from "./router";
import { SettingsClients } from "./settings";

type AppProps = {
  readonly initialPath?: AppRoutePath | undefined;
  readonly queryClient?: QueryClient | undefined;
  readonly sources?: ScreenSources | undefined;
  readonly settingsClient?: SettingsClient | undefined;
};

const coreScreenSources: ScreenSources = {
  ...pendingScreenSources,
  briefings: getBriefings,
};

const CoreEventBridge = (): null => {
  useCoreEvent();
  return null;
};

const AppErrorFallback = (props: Readonly<FallbackProps>): ReactElement => (
  <Page title={text("app.title", "ko")}>
    <EmptyState
      title={text("app.errorTitle", "ko")}
      action={{
        label: text("app.retry", "ko"),
        onPress: (): void => {
          props.resetErrorBoundary();
        },
      }}
    >
      {text("app.errorBody", "ko")}
    </EmptyState>
  </Page>
);

export const App = (props: AppProps): ReactElement => {
  const [defaultClient] = useState(createAppQueryClient);
  const client = props.queryClient ?? defaultClient;
  const sources = props.sources ?? coreScreenSources;
  return (
    <ErrorBoundary FallbackComponent={AppErrorFallback}>
      <QueryClientProvider client={client}>
        <CoreEventBridge />
        {props.settingsClient === undefined ? (
          <AppRouter
            initialPath={props.initialPath}
            sources={sources}
            system="hidden"
          />
        ) : (
          <OnboardingClientProvider client={props.settingsClient}>
            <SettingsClients client={props.settingsClient}>
              <FirstRunGate>
                <AppRouter
                  initialPath={props.initialPath}
                  sources={sources}
                  system="shown"
                />
              </FirstRunGate>
            </SettingsClients>
          </OnboardingClientProvider>
        )}
      </QueryClientProvider>
    </ErrorBoundary>
  );
};
