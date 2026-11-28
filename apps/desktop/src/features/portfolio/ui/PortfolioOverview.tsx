import type { ReactElement } from "react";
import {
  Card,
  EmptyState,
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
import { OverviewColumns } from "./OverviewColumns";
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

const overviewParts = (
  props: Props,
): Pick<Props, "reports" | "briefing" | "signals"> => ({
  reports: (
    <Stack gap="space-4">
      {props.assessment}
      {props.reports}
    </Stack>
  ),
  briefing: (
    <Stack position={{ topic: "daily-briefing" }}>{props.briefing}</Stack>
  ),
  signals: <Stack position={{ topic: "signals" }}>{props.signals}</Stack>,
});

const OverviewReady = (
  props: Props & { readonly view: PortfolioView; readonly retry: () => void },
): ReactElement => {
  const parts = overviewParts(props);
  return (
    <>
      {props.view.connection === "stored" ? (
        <StatusBar tone="info">{portfolioText("storedNotice", "ko")}</StatusBar>
      ) : null}
      <OverviewColumns
        overview={<SummaryCard view={props.view} />}
        positions={
          <Stack position={{ topic: "positions" }}>
            <HoldingsCard
              holdings={props.view.holdings}
              onOpenInstrument={props.onOpenInstrument}
              onOpenTransactions={props.onOpenTransactions}
            />
          </Stack>
        }
        allocation={
          <Stack position={{ topic: "allocation" }}>
            <AllocationSection
              load={props.sources.allocation}
              retry={props.retry}
            />
          </Stack>
        }
        reports={parts.reports}
        briefing={parts.briefing}
        signals={parts.signals}
      />
    </>
  );
};

const OverviewPending = (props: {
  readonly status: ReactElement;
  readonly overview: Props;
}): ReactElement => {
  const parts = overviewParts(props.overview);
  return (
    <OverviewColumns
      overview={<Stack position={{ topic: "positions" }}>{props.status}</Stack>}
      positions={undefined}
      allocation={undefined}
      reports={parts.reports}
      briefing={parts.briefing}
      signals={parts.signals}
    />
  );
};

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
