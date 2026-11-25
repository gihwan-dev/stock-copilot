import type { ReactElement } from "react";
import {
  Card,
  EmptyState,
  Skeleton,
  Stack,
  StatusBar,
} from "../../../shared/ds";
import type { PortfolioView, ScreenSources } from "../../../shared/ipc";
import { text } from "../../../shared/lib";
import { portfolioText } from "../messages";
import { usePortfolio } from "../model/portfolio";
import { AllocationSection } from "./AllocationSection";
import { HoldingsCard } from "./HoldingsCard";
import { SummaryCards } from "./SummaryCards";

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
      return portfolio.view.failure.kind === "command" &&
        portfolio.view.failure.error.kind === "user-fixable" ? (
        <EmptyState title={portfolioText("errorTitle", "ko")} action={action}>
          {portfolioText("errorAccessBody", "ko")}
        </EmptyState>
      ) : (
        <EmptyState title={portfolioText("errorTitle", "ko")} action={action} />
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
