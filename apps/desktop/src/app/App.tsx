import { QueryClientProvider, type QueryClient } from "@tanstack/react-query";
import { useState, type ReactElement } from "react";
import { ErrorBoundary, type FallbackProps } from "react-error-boundary";
import { FirstRunGate, OnboardingClientProvider } from "../features/onboarding";
import { EmptyState, Page } from "../shared/ds";
import { unavailableSettingsClient, type SettingsClient } from "../shared/ipc";
import { text, type AppRoutePath } from "../shared/lib";
import { createAppQueryClient } from "./query";
import { AppRouter } from "./router";
import { SettingsClients } from "./settings";

type AppProps = {
  readonly initialPath?: AppRoutePath | undefined;
  readonly queryClient?: QueryClient | undefined;
  readonly settingsClient?: SettingsClient | undefined;
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
  return (
    <ErrorBoundary FallbackComponent={AppErrorFallback}>
      <QueryClientProvider client={client}>
        <OnboardingClientProvider
          client={props.settingsClient ?? unavailableSettingsClient}
        >
          <SettingsClients
            client={props.settingsClient ?? unavailableSettingsClient}
          >
            <FirstRunGate>
              <AppRouter initialPath={props.initialPath} />
            </FirstRunGate>
          </SettingsClients>
        </OnboardingClientProvider>
      </QueryClientProvider>
    </ErrorBoundary>
  );
};
