import {
  Outlet,
  RouterProvider,
  createHashHistory,
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
} from "@tanstack/react-router";
import { useCallback, useState, type ReactElement } from "react";
import type { ScreenSources } from "../shared/ipc";
import type { AppRoutePath } from "../shared/lib";
import { useNavigationEvent } from "../shared/platform";
import {
  ScreenProviders,
  type NavigateFn,
  type SystemSettingsPresence,
} from "./context";
import {
  BriefingsRouteView,
  HomeView,
  SettingsRouteView,
  ShellLayout,
  StatusView,
  TransactionsView,
} from "./views";

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
  component: HomeView,
});

const transactionsRoute = createRoute({
  getParentRoute: () => shellLayoutRoute,
  path: "/transactions",
  component: TransactionsView,
});

const briefingsRoute = createRoute({
  getParentRoute: () => shellLayoutRoute,
  path: "/briefings",
  component: BriefingsRouteView,
});

const settingsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/settings",
  component: SettingsRouteView,
});

const statusRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/settings/status",
  component: StatusView,
});

const routeTree = rootRoute.addChildren([
  shellLayoutRoute.addChildren([homeRoute, transactionsRoute, briefingsRoute]),
  settingsRoute,
  statusRoute,
]);

export const AppRouter = (props: {
  readonly initialPath?: AppRoutePath | undefined;
  readonly sources: ScreenSources;
  readonly system: SystemSettingsPresence;
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
  const openDataStatus = useCallback((): void => {
    router.history.push("/settings/status");
  }, [router]);
  useNavigationEvent(openDataStatus);
  return (
    <ScreenProviders go={go} sources={props.sources} system={props.system}>
      <RouterProvider router={router} />
    </ScreenProviders>
  );
};
