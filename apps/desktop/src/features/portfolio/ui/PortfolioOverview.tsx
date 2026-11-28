import type { ReactElement } from "react";
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
import { text, type EntityId } from "../../../shared/lib";
import { portfolioText, type PortfolioMessageKey } from "../messages";
import { usePortfolio } from "../model/portfolio";
import { AllocationSection } from "./AllocationSection";
import { HoldingsCard } from "./HoldingsCard";
import { SummaryCards } from "./SummaryCards";

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

type Aside = {
  readonly top: ReactElement;
  readonly bottom: ReactElement;
};

type Props = {
  readonly sources: Pick<ScreenSources, "portfolio" | "allocation">;
  readonly onOpenInstrument: (symbol: EntityId) => void;
  readonly onOpenTransactions: () => void;
  readonly assessment: ReactElement;
  readonly reports: ReactElement;
  readonly aside: Aside;
};

const OverviewReady = (props: {
  readonly view: PortfolioView;
  readonly sources: Props["sources"];
  readonly retry: () => void;
  readonly onOpenInstrument: (symbol: EntityId) => void;
  readonly onOpenTransactions: () => void;
  readonly assessment: ReactElement;
  readonly reports: ReactElement;
  readonly aside: Aside;
}): ReactElement => (
  <>
    {props.view.connection === "stored" ? (
      <StatusBar tone="info">{portfolioText("storedNotice", "ko")}</StatusBar>
    ) : null}
    <SummaryCards view={props.view} />
    <Grid layout="fit-row">
      <GridItem span={{ narrow: 12, medium: 8 }}>
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
          {props.reports}
        </Stack>
      </GridItem>
      <GridItem span={{ narrow: 12, medium: 4 }}>
        <Stack gap="space-4">
          {props.aside.top}
          {props.aside.bottom}
        </Stack>
      </GridItem>
    </Grid>
  </>
);

const OverviewPending = (props: {
  readonly status: ReactElement;
  readonly overview: Props;
}): ReactElement => (
  <>
    {props.status}
    {props.overview.assessment}
    {props.overview.reports}
    <Grid layout="fit-row">
      <GridItem span={{ narrow: 12, medium: 6 }}>
        {props.overview.aside.top}
      </GridItem>
      <GridItem span={{ narrow: 12, medium: 6 }}>
        {props.overview.aside.bottom}
      </GridItem>
    </Grid>
  </>
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
          aside={props.aside}
        />
      );
  }
};
