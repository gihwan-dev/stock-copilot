import { useQueryClient } from "@tanstack/react-query";
import type { ReactElement } from "react";
import { AboutSection } from "../features/about";
import { BriefingCard, BriefingHistoryScreen } from "../features/briefing";
import { ChatWorkspace } from "../features/chat";
import { useDashboardResearch } from "../features/research";
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
  Card,
  Disclosure,
  Text,
  EmptyState,
  Page,
  Skeleton,
  Stack,
} from "../shared/ds";
import { coreUpdateClient, getDataStatus } from "../shared/ipc";
import {
  dockPreferencesScope,
  type DockSources,
  useDockPreferences,
  queryKeys,
  text,
} from "../shared/lib";
import {
  detach,
  useDashboardDockPreferences,
  useDockPersistence,
  showDockMenu,
  applyDockAction,
} from "../shared/platform";
import {
  useNavigate,
  useInstrumentSelection,
  useSources,
  useSettingsOrigin,
  useSystemSettings,
} from "./context";
import { SettingsSections } from "./settings";
import { describeIpcFailure, useAppVersion } from "./query";

export const ChatRouteView = (): ReactElement => (
  <ChatWorkspace>
    {(conversation): ReactElement => (
      <Page
        title={text("shell.chat", "ko")}
        layout="fill"
        conversation={conversation}
      >
        <Disclosure title={text("notice.chatDataShort", "ko")}>
          <Text>{text("notice.chatData", "ko")}</Text>
        </Disclosure>
      </Page>
    )}
  </ChatWorkspace>
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

const HomeBriefing = (): ReactElement => {
  const go = useNavigate();
  const sources = useSources();
  return (
    <BriefingCard
      presentation="summary"
      load={sources.briefings}
      onOpenHistory={(): void => {
        go("/briefing");
      }}
    />
  );
};

const StartupNotice = (props: {
  readonly view: Exclude<ReturnType<typeof useAppVersion>, { status: "ready" }>;
  readonly retry: () => void;
}): ReactElement =>
  props.view.status === "loading" ? (
    <Card title={text("app.loading", "ko")}>
      <Skeleton variant="text" />
    </Card>
  ) : (
    <EmptyState
      title={text("app.errorTitle", "ko")}
      action={{ label: text("app.retry", "ko"), onPress: props.retry }}
    >
      {describeIpcFailure(props.view.failure)}
    </EmptyState>
  );

const DockPreferences = dockPreferencesScope.Context;

const DashboardView = ({
  preferences,
}: {
  readonly preferences: DockSources;
}): ReactElement => {
  const go = useNavigate();
  const sources = useSources();
  const open = useInstrumentSelection();
  const research = useDashboardResearch(open);
  const view = useAppVersion();
  const dock = useDockPreferences(preferences, showDockMenu, applyDockAction);
  useDockPersistence(dock.model, dock.save);
  const queryClient = useQueryClient();
  const retry = (): void => {
    detach({
      kind: "invalidate-queries",
      client: queryClient,
      keys: [queryKeys.appVersion],
    });
  };
  return (
    <DockPreferences value={dock}>
      <PortfolioOverview
        sources={sources}
        startup={
          view.status === "ready" ? undefined : (
            <StartupNotice view={view} retry={retry} />
          )
        }
        onOpenInstrument={open}
        onOpenTransactions={(): void => {
          go("/transactions");
        }}
        reports={{
          holdings: research("holdings"),
          candidates: research("candidates"),
          economy: research("economy"),
        }}
        signals={
          <SignalCards
            sources={sources}
            footer={<ScorecardPanel load={sources.scorecard} />}
          />
        }
        briefing={<HomeBriefing />}
      />
    </DockPreferences>
  );
};
export const HomeView = (): ReactElement => {
  const preferences = useDashboardDockPreferences();
  return <DashboardView key={preferences.profile} preferences={preferences} />;
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
  const origin = useSettingsOrigin();
  const tossEntry = useKeyEntry();
  return (
    <SettingsScreen
      tossEntry={tossEntry}
      backTarget={origin === "/" ? "home" : "previous"}
      onBack={(): void => {
        go(origin);
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
  );
};

export const StatusView = (): ReactElement => {
  const go = useNavigate();
  const sources = useSources();
  return (
    <StatusScreen
      load={sources.status}
      loadData={getDataStatus}
      onBack={(): void => {
        go("/settings");
      }}
    />
  );
};
