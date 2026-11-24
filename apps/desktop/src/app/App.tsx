import { QueryClientProvider, type QueryClient } from "@tanstack/react-query";
import { useState, type ReactElement } from "react";
import { ErrorBoundary, type FallbackProps } from "react-error-boundary";
import { EmptyState, Page } from "../shared/ds";
import { text, type AppRoutePath } from "../shared/lib";
import { useCoreEvent } from "../shared/platform";
import { createAppQueryClient } from "./query";
import { AppRouter } from "./router";

type AppProps = {
  readonly initialPath?: AppRoutePath | undefined;
  readonly queryClient?: QueryClient | undefined;
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
  return (
    <ErrorBoundary FallbackComponent={AppErrorFallback}>
      <QueryClientProvider client={client}>
        <CoreEventBridge />
        <AppRouter initialPath={props.initialPath} />
      </QueryClientProvider>
    </ErrorBoundary>
  );
};
