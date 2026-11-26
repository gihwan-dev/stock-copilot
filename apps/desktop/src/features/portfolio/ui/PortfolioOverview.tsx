import type { ReactElement } from "react";
import {
  Card,
  EmptyState,
  Grid,
  GridItem,
  Skeleton,
  StatusBar,
} from "../../../shared/ds";
import type {
  CommandFailureReason,
  IpcFailure,
  PortfolioView,
  ScreenSources,
} from "../../../shared/ipc";
import { text } from "../../../shared/lib";
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
  readonly onOpenTransactions: () => void;
  readonly aside: Aside;
};

const OverviewReady = (props: {
  readonly view: PortfolioView;
  readonly sources: Props["sources"];
  readonly retry: () => void;
  readonly onOpenTransactions: () => void;
  readonly aside: Aside;
}): ReactElement => (
  <>
    {props.view.connection === "stored" ? (
      <StatusBar tone="info">{portfolioText("storedNotice", "ko")}</StatusBar>
    ) : null}
    <SummaryCards view={props.view} />
    <Grid layout="fill-row">
      <GridItem span={8}>
        <HoldingsCard
          holdings={props.view.holdings}
          onOpenTransactions={props.onOpenTransactions}
        />
      </GridItem>
      <GridItem span={4}>{props.aside.top}</GridItem>
    </Grid>
    <Grid layout="fill-row">
      <GridItem span={8}>
        <AllocationSection
          load={props.sources.allocation}
          retry={props.retry}
        />
      </GridItem>
      <GridItem span={4}>{props.aside.bottom}</GridItem>
    </Grid>
  </>
);

const OverviewPending = (props: {
  readonly status: ReactElement;
  readonly aside: Aside;
}): ReactElement => (
  <>
    {props.status}
    <Grid layout="fill-row">
      <GridItem span={6}>{props.aside.top}</GridItem>
      <GridItem span={6}>{props.aside.bottom}</GridItem>
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
          aside={props.aside}
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
          aside={props.aside}
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
          aside={props.aside}
        />
      );
    case "ready":
      return (
        <OverviewReady
          view={portfolio.view.data}
          sources={props.sources}
          retry={portfolio.retry}
          onOpenTransactions={props.onOpenTransactions}
          aside={props.aside}
        />
      );
  }
};
