import { useQueryClient } from "@tanstack/react-query";
import {
  Outlet,
  RouterProvider,
  createHashHistory,
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
} from "@tanstack/react-router";
import {
  createContext,
  use,
  useCallback,
  useState,
  type ReactElement,
} from "react";
import { SettingsScreen } from "../features/settings";
import {
  AppShell,
  Card,
  EmptyState,
  Page,
  Section,
  Skeleton,
  Stack,
  Text,
} from "../shared/ds";
import { queryKeys, text, type AppRoutePath } from "../shared/lib";
import { detach } from "../shared/platform";
import { describeIpcFailure, useAppVersion } from "./query";

type NavigateFn = (path: AppRoutePath) => void;

const NavigationContext = createContext<NavigateFn>((): void => undefined);

const SidebarPlaceholder = (): ReactElement => (
  <Stack gap="space-2">
    <Text variant="caption" tone="ink-muted">
      {text("app.stockListEmpty", "ko")}
    </Text>
  </Stack>
);

const ChatPlaceholder = (): ReactElement => (
  <Stack gap="space-2">
    <Text variant="label">{text("shell.chat", "ko")}</Text>
    <Text variant="body-sm" tone="ink-muted">
      {text("app.chatReady", "ko")}
    </Text>
  </Stack>
);

const HomeBody = (props: { readonly retry: () => void }): ReactElement => {
  const view = useAppVersion();
  switch (view.status) {
    case "loading":
      return (
        <Card title={text("app.loading", "ko")}>
          <Skeleton variant="text" />
        </Card>
      );
    case "error":
      return (
        <EmptyState
          title={text("app.errorTitle", "ko")}
          action={{ label: text("app.retry", "ko"), onPress: props.retry }}
        >
          {describeIpcFailure(view.failure)}
        </EmptyState>
      );
    case "ready":
      return (
        <Card title={text("app.title", "ko")}>
          <Text>{text("app.readySummary", "ko")}</Text>
        </Card>
      );
  }
};

const HomeSkeletonView = (): ReactElement => {
  const go = use(NavigationContext);
  const queryClient = useQueryClient();
  const openSettings = (): void => {
    go("/settings");
  };
  const retry = (): void => {
    detach({
      kind: "invalidate-queries",
      client: queryClient,
      keys: [queryKeys.appVersion],
    });
  };
  return (
    <Page
      title={text("app.homeTitle", "ko")}
      actions={{ label: text("app.openSettings", "ko"), onPress: openSettings }}
    >
      <Section title={text("app.sectionOverview", "ko")}>
        <HomeBody retry={retry} />
      </Section>
    </Page>
  );
};

const TransactionsPlaceholder = (): ReactElement => {
  const go = use(NavigationContext);
  return (
    <Page
      title={text("app.transactionsTitle", "ko")}
      actions={{
        label: text("app.backToHome", "ko"),
        onPress: (): void => {
          go("/");
        },
      }}
    >
      <EmptyState title={text("app.ipcNoData", "ko")} />
    </Page>
  );
};

const SettingsRouteView = (): ReactElement => {
  const go = use(NavigationContext);
  return (
    <SettingsScreen
      onBack={(): void => {
        go("/");
      }}
      onOpenStatus={(): void => {
        go("/settings/status");
      }}
    />
  );
};

const StatusPlaceholder = (): ReactElement => {
  const go = use(NavigationContext);
  return (
    <Page
      title={text("app.statusTitle", "ko")}
      actions={{
        label: text("app.openSettings", "ko"),
        onPress: (): void => {
          go("/settings");
        },
      }}
    >
      <EmptyState title={text("app.ipcNoData", "ko")} />
    </Page>
  );
};

const ShellLayout = (): ReactElement => (
  <AppShell list={<SidebarPlaceholder />} chat={<ChatPlaceholder />}>
    <Outlet />
  </AppShell>
);

const rootRoute = createRootRoute({
  component: (): ReactElement => <Outlet />,
});

const shellLayoutRoute = createRoute({
  getParentRoute: () => rootRoute,
  id: "shell",
  component: ShellLayout,
});

const homeRoute = createRoute({
  getParentRoute: () => shellLayoutRoute,
  path: "/",
  component: HomeSkeletonView,
});

const transactionsRoute = createRoute({
  getParentRoute: () => shellLayoutRoute,
  path: "/transactions",
  component: TransactionsPlaceholder,
});

const settingsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/settings",
  component: SettingsRouteView,
});

const statusRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/settings/status",
  component: StatusPlaceholder,
});

const routeTree = rootRoute.addChildren([
  shellLayoutRoute.addChildren([homeRoute, transactionsRoute]),
  settingsRoute,
  statusRoute,
]);

export const AppRouter = (props: {
  readonly initialPath?: AppRoutePath | undefined;
}): ReactElement => {
  const [router] = useState(() =>
    createRouter({
      routeTree,
      history:
        props.initialPath === undefined
          ? createHashHistory()
          : createMemoryHistory({ initialEntries: [props.initialPath] }),
    }),
  );
  const go = useCallback<NavigateFn>(
    (path): void => {
      router.history.push(path);
    },
    [router],
  );
  return (
    <NavigationContext value={go}>
      <RouterProvider router={router} />
    </NavigationContext>
  );
};
