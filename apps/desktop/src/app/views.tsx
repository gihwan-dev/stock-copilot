import { useQueryClient } from "@tanstack/react-query";
import type { ReactElement } from "react";
import { AboutSection } from "../features/about";
import { BriefingCard, BriefingHistoryScreen } from "../features/briefing";
import { ChatPanel } from "../features/chat";
import { PortfolioOverview } from "../features/portfolio";
import { useKeyEntry } from "../features/onboarding";
import { SettingsScreen } from "../features/settings";
import { RiskEditors } from "../features/settings-risk";
import { ScorecardPanel } from "../features/scorecard";
import { SignalCards } from "../features/signal";
import { StatusScreen } from "../features/status";
import { TransactionsScreen } from "../features/transactions";
import { UpdateCard } from "../features/updates";
import {
  AppShell,
  Card,
  Disclosure,
  Text,
  EmptyState,
  Page,
  Skeleton,
  Stack,
} from "../shared/ds";
import { coreUpdateClient, getDataStatus } from "../shared/ipc";
import { queryKeys, text } from "../shared/lib";
import { detach } from "../shared/platform";
import { useNavigate, useSources, useSystemSettings } from "./context";
import { SettingsSections } from "./settings";
import { describeIpcFailure, useAppVersion } from "./query";

export const ChatRouteView = (): ReactElement => (
  <Page title={text("shell.chat", "ko")}>
    <Disclosure title={text("notice.chatDataShort", "ko")}>
      <Text>{text("notice.chatData", "ko")}</Text>
    </Disclosure>
    <ChatPanel context={undefined} />
  </Page>
);

export const LatestBriefingView = (): ReactElement => {
  const sources = useSources();
  const go = useNavigate();
  return (
    <Page title={text("nav.briefing", "ko")}>
      <BriefingCard
        load={sources.briefings}
        onOpenHistory={(): void => {
          go("/briefings");
        }}
      />
    </Page>
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
          aside={{
            top: (
              <SignalCards
                sources={sources}
                footer={<ScorecardPanel load={sources.scorecard} />}
              />
            ),
            bottom: (
              <BriefingCard
                layout="fill"
                presentation="summary"
                load={sources.briefings}
                onOpenHistory={(): void => {
                  go("/briefing");
                }}
              />
            ),
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
      layout="fill"
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

export const BriefingsRouteView = (): ReactElement => {
  const go = useNavigate();
  const sources = useSources();
  return (
    <BriefingHistoryScreen
      load={sources.briefings}
      onBack={(): void => {
        go("/");
      }}
    />
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
  const sources = useSources();
  const system = useSystemSettings();
  const tossEntry = useKeyEntry();
  return (
    <AppShell>
      <SettingsScreen
        tossEntry={tossEntry}
        onBack={(): void => {
          go("/");
        }}
        onOpenStatus={(): void => {
          go("/settings/status");
        }}
        summaryScope={system === "shown" ? "status-only" : "all"}
        advanced={[
          ...(system === "shown" ? [<SettingsSections key="sections" />] : []),
          <RiskEditors key="risk-editors" sources={sources} />,
        ]}
        system={
          <Stack key="system" gap="space-8">
            {[
              <UpdateCard key="updates" client={coreUpdateClient} />,
              <AboutSection key="about" />,
            ]}
          </Stack>
        }
      />
    </AppShell>
  );
};

export const StatusView = (): ReactElement => {
  const go = useNavigate();
  const sources = useSources();
  return (
    <AppShell>
      <StatusScreen
        load={sources.status}
        loadData={getDataStatus}
        onBack={(): void => {
          go("/settings");
        }}
      />
    </AppShell>
  );
};
