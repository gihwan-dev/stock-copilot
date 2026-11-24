import { useQueryClient } from "@tanstack/react-query";
import { Outlet } from "@tanstack/react-router";
import type { ReactElement } from "react";
import { PortfolioOverview } from "../features/portfolio";
import { SelfCheckBar } from "../features/self-check";
import { SettingsScreen } from "../features/settings";
import { StatusScreen } from "../features/status";
import { TransactionsScreen } from "../features/transactions";
import {
  AppShell,
  Card,
  EmptyState,
  Page,
  Skeleton,
  Stack,
  Text,
} from "../shared/ds";
import { queryKeys, text } from "../shared/lib";
import { detach } from "../shared/platform";
import { useNavigate, useSources, useSystemSettings } from "./context";
import { SettingsSections } from "./settings";
import { describeIpcFailure, useAppVersion } from "./query";

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

export const ShellLayout = (): ReactElement => {
  const go = useNavigate();
  const sources = useSources();
  return (
    <AppShell
      list={<SidebarPlaceholder />}
      chat={<ChatPlaceholder />}
      status={
        <SelfCheckBar
          load={sources.selfCheck}
          onOpenSettings={(): void => {
            go("/settings");
          }}
        />
      }
    >
      <Outlet />
    </AppShell>
  );
};

const HomeBody = (props: { readonly retry: () => void }): ReactElement => {
  const go = useNavigate();
  const sources = useSources();
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
        <PortfolioOverview
          sources={sources}
          onOpenTransactions={(): void => {
            go("/transactions");
          }}
        />
      );
  }
};

export const HomeView = (): ReactElement => {
  const go = useNavigate();
  const queryClient = useQueryClient();
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
      actions={{
        label: text("app.openSettings", "ko"),
        onPress: (): void => {
          go("/settings");
        },
      }}
    >
      <HomeBody retry={retry} />
    </Page>
  );
};

export const TransactionsView = (): ReactElement => {
  const go = useNavigate();
  const sources = useSources();
  return (
    <TransactionsScreen
      load={sources.transactions}
      onBack={(): void => {
        go("/");
      }}
    />
  );
};

export const SettingsRouteView = (): ReactElement => {
  const go = useNavigate();
  const system = useSystemSettings();
  return (
    <SettingsScreen
      onBack={(): void => {
        go("/");
      }}
      onOpenStatus={(): void => {
        go("/settings/status");
      }}
      system={system === "shown" ? <SettingsSections key="system" /> : null}
    />
  );
};

export const StatusView = (): ReactElement => {
  const go = useNavigate();
  const sources = useSources();
  return (
    <StatusScreen
      load={sources.status}
      onBack={(): void => {
        go("/settings");
      }}
    />
  );
};
