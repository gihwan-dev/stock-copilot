import type { ReactElement } from "react";
import {
  Card,
  EmptyState,
  Skeleton,
  Stack,
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

type Props = {
  readonly sources: Pick<ScreenSources, "portfolio" | "allocation">;
  readonly onOpenTransactions: () => void;
};

const OverviewReady = (props: {
  readonly view: PortfolioView;
  readonly sources: Props["sources"];
  readonly retry: () => void;
  readonly onOpenTransactions: () => void;
}): ReactElement => (
  <Stack gap="space-6">
    {[
      ...(props.view.connection === "stored"
        ? [
            <StatusBar key="stored" tone="info">
              {portfolioText("storedNotice", "ko")}
            </StatusBar>,
          ]
        : []),
      <Stack key="overview" gap="space-4">
        <SummaryCards view={props.view} />
        <HoldingsCard
          holdings={props.view.holdings}
          onOpenTransactions={props.onOpenTransactions}
        />
      </Stack>,
      <AllocationSection
        key="allocation"
        load={props.sources.allocation}
        retry={props.retry}
      />,
    ]}
  </Stack>
);

export const PortfolioOverview = (props: Props): ReactElement => {
  const portfolio = usePortfolio(props.sources.portfolio);
  switch (portfolio.view.status) {
    case "loading":
      return (
        <Card title={portfolioText("loadingTitle", "ko")}>
          <Skeleton variant="text" lines={3} />
        </Card>
      );
    case "empty":
      return (
        <EmptyState title={text("app.readySummary", "ko")}>
          {portfolioText("emptyBody", "ko")}
        </EmptyState>
      );
    case "error": {
      const action = {
        label: portfolioText("retry", "ko"),
        onPress: portfolio.retry,
      };
      return (
        <EmptyState title={portfolioText("errorTitle", "ko")} action={action}>
          {portfolioText(failureBody(portfolio.view.failure), "ko")}
        </EmptyState>
      );
    }
    case "ready":
      return (
        <OverviewReady
          view={portfolio.view.data}
          sources={props.sources}
          retry={portfolio.retry}
          onOpenTransactions={props.onOpenTransactions}
        />
      );
  }
};
