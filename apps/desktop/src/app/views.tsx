import { useQueryClient } from "@tanstack/react-query";
import { Outlet } from "@tanstack/react-router";
import { useState, type ReactElement } from "react";
import { AboutSection } from "../features/about";
import { BriefingCard, BriefingHistoryScreen } from "../features/briefing";
import { ChatSession } from "../features/chat";
import { InstrumentDetail, InstrumentList } from "../features/instruments";
import { PortfolioOverview } from "../features/portfolio";
import { SelfCheckBar } from "../features/self-check";
import { useKeyEntry } from "../features/onboarding";
import { SettingsScreen } from "../features/settings";
import { RiskEditors } from "../features/settings-risk";
import { ScorecardPanel } from "../features/scorecard";
import { SignalCards } from "../features/signal";
import { StatusScreen } from "../features/status";
import { TransactionsScreen } from "../features/transactions";
import { UpdateBar, UpdateCard } from "../features/updates";
import {
  AppShell,
  Card,
  EmptyState,
  Page,
  Skeleton,
  Stack,
} from "../shared/ds";
import { coreUpdateClient } from "../shared/ipc";
import { queryKeys, text, type EntityId } from "../shared/lib";
import { detach } from "../shared/platform";
import { useNavigate, useSources, useSystemSettings } from "./context";
import { SettingsSections } from "./settings";
import { describeIpcFailure, useAppVersion } from "./query";

const chatNotice = {
  summary: text("notice.chatDataShort", "ko"),
  detail: text("notice.chatData", "ko"),
};

export const ShellLayout = (): ReactElement => {
  const go = useNavigate();
  const sources = useSources();
  const [shown, setShown] = useState<EntityId | undefined>(undefined);
  const list = <InstrumentList sources={sources} onSelect={setShown} />;
  const status = (
    <>
      <UpdateBar client={coreUpdateClient} />
      <SelfCheckBar
        load={sources.selfCheck}
        onOpenSettings={(): void => {
          setShown(undefined);
          go("/settings");
        }}
      />
    </>
  );
  const page =
    shown === undefined ? (
      <Outlet />
    ) : (
      <InstrumentDetail
        key={shown}
        sources={sources}
        instrument={shown}
        onClose={(): void => {
          setShown(undefined);
        }}
      />
    );
  return (
    <ChatSession context={undefined}>
      {(parts): ReactElement => (
        <AppShell
          list={list}
          chat={parts.messages}
          chatComposer={parts.composer}
          chatNotice={chatNotice}
          status={status}
        >
          {page}
        </AppShell>
      )}
    </ChatSession>
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
        <Stack gap="space-6">
          <PortfolioOverview
            sources={sources}
            onOpenTransactions={(): void => {
              go("/transactions");
            }}
          />
          <SignalCards sources={sources} />
          <BriefingCard
            load={sources.briefings}
            onOpenHistory={(): void => {
              go("/briefings");
            }}
          />
          <ScorecardPanel load={sources.scorecard} />
        </Stack>
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
        onBack={(): void => {
          go("/settings");
        }}
      />
    </AppShell>
  );
};
