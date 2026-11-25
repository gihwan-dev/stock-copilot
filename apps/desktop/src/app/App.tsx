import { QueryClientProvider, type QueryClient } from "@tanstack/react-query";
import { useState, type ReactElement } from "react";
import { ErrorBoundary, type FallbackProps } from "react-error-boundary";
import { FirstRunGate, OnboardingClientProvider } from "../features/onboarding";
import { AppFrame, AppShell, EmptyState, Page } from "../shared/ds";
import {
  coreSetupClient,
  getAllocation,
  getBriefings,
  getPortfolio,
  investorProfile,
  orderSettings,
  pendingScreenSources,
  saveInvestorProfile,
  saveOrderSettings,
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
  readonly setupClient?: SettingsClient | undefined;
};

const coreScreenSources: ScreenSources = {
  ...pendingScreenSources,
  briefings: getBriefings,
  portfolio: getPortfolio,
  allocation: getAllocation,
  investorProfile,
  orderSettings,
  saveInvestorProfile,
  saveOrderSettings,
};

const CoreEventBridge = (): null => {
  useCoreEvent();
  return null;
};

const AppErrorFallback = (props: Readonly<FallbackProps>): ReactElement => (
  <AppShell>
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
  </AppShell>
);

const SetupGate = (props: {
  readonly client: SettingsClient | undefined;
  readonly children: ReactElement;
}): ReactElement =>
  props.client === undefined ? (
    props.children
  ) : (
    <OnboardingClientProvider client={props.client}>
      <FirstRunGate>{props.children}</FirstRunGate>
    </OnboardingClientProvider>
  );

export const App = (props: AppProps): ReactElement => {
  const [defaultClient] = useState(createAppQueryClient);
  const client = props.queryClient ?? defaultClient;
  const sources = props.sources ?? coreScreenSources;
  return (
    <AppFrame>
      <ErrorBoundary FallbackComponent={AppErrorFallback}>
        <QueryClientProvider client={client}>
          <CoreEventBridge />
          {props.settingsClient === undefined ? (
            <SetupGate client={props.setupClient}>
              <AppRouter
                initialPath={props.initialPath}
                sources={sources}
                system="hidden"
              />
            </SetupGate>
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
    </AppFrame>
  );
};

export const CoreApp = (): ReactElement => (
  <App setupClient={coreSetupClient} />
);
