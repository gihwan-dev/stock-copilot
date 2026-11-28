import type { ReactElement } from "react";
import {
  Card,
  EmptyState,
  Skeleton,
  StatusBar,
  Page,
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
  readonly reports: Readonly<
    Record<"holdings" | "candidates" | "economy", ReactElement>
  >;
  readonly briefing: ReactElement;
  readonly signals: ReactElement;
  readonly startup: ReactElement | undefined;
};

const accountSections = (
  view: PortfolioView,
  props: Props,
  retry: () => void,
): Readonly<Record<"overview" | "positions" | "allocation", ReactElement>> => ({
  overview: <SummaryCard view={view} />,
  positions: (
    <HoldingsCard
      holdings={view.holdings}
      onOpenInstrument={props.onOpenInstrument}
      onOpenTransactions={props.onOpenTransactions}
    />
  ),
  allocation: (
    <AllocationSection load={props.sources.allocation} retry={retry} />
  ),
});

const AccountNotice = (props: {
  readonly state: Exclude<
    ReturnType<typeof usePortfolio>["view"],
    { status: "ready" }
  >;
}): ReactElement => {
  switch (props.state.status) {
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
    case "error":
      return (
        <EmptyState title={portfolioText("errorTitle", "ko")}>
          {portfolioText(failureBody(props.state.failure), "ko")}
        </EmptyState>
      );
  }
};

const unavailableSections = (
  state: Exclude<ReturnType<typeof usePortfolio>["view"], { status: "ready" }>,
): ReturnType<typeof accountSections> => {
  const notice = <AccountNotice state={state} />;
  return { overview: notice, positions: notice, allocation: notice };
};

export const PortfolioOverview = (props: Props): ReactElement => {
  const portfolio = usePortfolio(
    props.sources.portfolio,
    props.startup === undefined ? "enabled" : "waiting",
  );
  const view = portfolio.view;
  if (props.startup !== undefined)
    return <Page title={text("app.homeTitle", "ko")}>{props.startup}</Page>;
  const sections =
    view.status === "ready"
      ? accountSections(view.data, props, portfolio.retry)
      : unavailableSections(view);
  return (
    <Page
      layout="workspace"
      actions={
        view.status === "error"
          ? { label: portfolioText("retry", "ko"), onPress: portfolio.retry }
          : undefined
      }
      title={text("app.homeTitle", "ko")}
      widgets={{
        ...sections,
        ...props.reports,
        briefing: props.briefing,
        signals: props.signals,
      }}
    >
      {view.status === "ready" && view.data.connection === "stored" ? (
        <StatusBar tone="info">{portfolioText("storedNotice", "ko")}</StatusBar>
      ) : undefined}
    </Page>
  );
};
