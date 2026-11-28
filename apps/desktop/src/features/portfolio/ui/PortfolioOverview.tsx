import { Fragment, type ReactElement } from "react";
import {
  Card,
  EmptyState,
  Grid,
  GridItem,
  Skeleton,
  StatusBar,
  Stack,
} from "../../../shared/ds";
import type {
  CommandFailureReason,
  IpcFailure,
  PortfolioView,
  ScreenSources,
} from "../../../shared/ipc";
import { useWideWindow } from "../../../shared/platform";
import { text, type EntityId } from "../../../shared/lib";
import { portfolioText, type PortfolioMessageKey } from "../messages";
import { usePortfolio } from "../model/portfolio";
import { AllocationSection } from "./AllocationSection";
import { HoldingsCard } from "./HoldingsCard";
import { SummaryCard } from "./SummaryCards";

const reasonBodies = {
  network: "errorNetworkBody",
  "server-busy": "errorServerBusyBody",
  "response-mismatch": "errorResponseMismatchBody",
  rejected: "errorRejectedBody",
  "instruments-not-ready": "errorInstrumentsBody",
  "access-denied": "errorAccessBody",
  unknown: "errorUnknownBody",
} as const satisfies Readonly<
  Record<CommandFailureReason, PortfolioMessageKey>
>;

const reasonBody = (
  reason: CommandFailureReason | null | undefined,
): PortfolioMessageKey =>
  reason === null || reason === undefined
    ? "errorUnknownBody"
    : reasonBodies[reason];

const failureBody = (failure: IpcFailure): PortfolioMessageKey => {
  switch (failure.kind) {
    case "command":
      return failure.error.kind === "user-fixable"
        ? "errorAccessBody"
        : reasonBody(failure.error.reason);
    case "framework":
    case "invalid-response":
    case "timeout":
      return "errorUnknownBody";
  }
};

type Props = {
  readonly sources: Pick<ScreenSources, "portfolio" | "allocation">;
  readonly onOpenInstrument: (symbol: EntityId) => void;
  readonly onOpenTransactions: () => void;
  readonly assessment: ReactElement;
  readonly reports: ReactElement;
  readonly briefing: ReactElement;
  readonly signals: ReactElement;
};

const OverviewColumns = (props: {
  readonly overview: ReactElement;
  readonly account: ReactElement;
  readonly reports: ReactElement;
  readonly briefing: ReactElement;
}): ReactElement => {
  const layout = useWideWindow();
  const span = layout === "wide" ? 6 : 12;
  const briefing = <Fragment key="briefing">{props.briefing}</Fragment>;
  return (
    <Grid layout="fit-row">
      <GridItem span={span}>
        <Stack gap="space-4">
          {[
            <Fragment key="summary">{props.overview}</Fragment>,
            ...(layout === "compact" ? [briefing] : []),
            <Fragment key="account">{props.account}</Fragment>,
          ]}
        </Stack>
      </GridItem>
      <GridItem span={span}>
        <Stack gap="space-4">
          {[
            ...(layout === "wide" ? [briefing] : []),
            <Fragment key="reports">{props.reports}</Fragment>,
          ]}
        </Stack>
      </GridItem>
    </Grid>
  );
};

const OverviewReady = (
  props: Props & {
    readonly view: PortfolioView;
    readonly retry: () => void;
  },
): ReactElement => (
  <>
    {props.view.connection === "stored" ? (
      <StatusBar tone="info">{portfolioText("storedNotice", "ko")}</StatusBar>
    ) : null}
    <OverviewColumns
      overview={<SummaryCard view={props.view} />}
      account={
        <Stack gap="space-4">
          <HoldingsCard
            holdings={props.view.holdings}
            onOpenInstrument={props.onOpenInstrument}
            onOpenTransactions={props.onOpenTransactions}
          />
          {props.assessment}
          <AllocationSection
            load={props.sources.allocation}
            retry={props.retry}
          />
          {props.signals}
        </Stack>
      }
      reports={props.reports}
      briefing={props.briefing}
    />
  </>
);

const OverviewPending = (props: {
  readonly status: ReactElement;
  readonly overview: Props;
}): ReactElement => (
  <OverviewColumns
    overview={props.status}
    account={
      <Stack gap="space-4">
        {props.overview.assessment}
        {props.overview.signals}
      </Stack>
    }
    reports={props.overview.reports}
    briefing={props.overview.briefing}
  />
);

export const PortfolioOverview = (props: Props): ReactElement => {
  const portfolio = usePortfolio(props.sources.portfolio);
  switch (portfolio.view.status) {
    case "loading":
      return (
        <OverviewPending
          status={
            <Card title={portfolioText("loadingTitle", "ko")}>
              <Skeleton variant="text" lines={3} />
            </Card>
          }
          overview={props}
        />
      );
    case "empty":
      return (
        <OverviewPending
          status={
            <EmptyState title={text("app.readySummary", "ko")}>
              {portfolioText("emptyBody", "ko")}
            </EmptyState>
          }
          overview={props}
        />
      );
    case "error":
      return (
        <OverviewPending
          status={
            <EmptyState
              title={portfolioText("errorTitle", "ko")}
              action={{
                label: portfolioText("retry", "ko"),
                onPress: portfolio.retry,
              }}
            >
              {portfolioText(failureBody(portfolio.view.failure), "ko")}
            </EmptyState>
          }
          overview={props}
        />
      );
    case "ready":
      return (
        <OverviewReady
          view={portfolio.view.data}
          sources={props.sources}
          retry={portfolio.retry}
          onOpenInstrument={props.onOpenInstrument}
          onOpenTransactions={props.onOpenTransactions}
          assessment={props.assessment}
          reports={props.reports}
          signals={props.signals}
          briefing={props.briefing}
        />
      );
  }
};
