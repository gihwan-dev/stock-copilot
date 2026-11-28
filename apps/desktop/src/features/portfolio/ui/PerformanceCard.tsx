import { useQuery } from "@tanstack/react-query";
import type { ReactElement } from "react";
import { Percent, PriceChange, Table, Text } from "../../../shared/ds";
import {
  getPerformance,
  type PerformanceLine,
  type PerformanceSpan,
  type PortfolioView,
  type SignedRate,
} from "../../../shared/ipc";
import { queryKeys, type UiText } from "../../../shared/lib";
import { portfolioText, type PortfolioMessageKey } from "../messages";

const TableRoot = Table.Root;
const TableBody = Table.Body;
const TableRowHeader = Table.RowHeader;
const TableCell = Table.Cell;
const TableRow = Table.Row;

const refreshMs = 600_000;

const spanKeys = {
  "1m": "performanceOneMonth",
  "3m": "performanceThreeMonths",
  "6m": "performanceSixMonths",
  "1y": "performanceOneYear",
} as const satisfies Record<PerformanceSpan, PortfolioMessageKey>;

const statusKeys = {
  waiting: "performanceWaiting",
  calculating: "performancePending",
  ready: "performanceUnavailable",
  "missing-price": "performanceMissingPrice",
  "missing-fx": "performanceMissingFx",
  "missing-cash": "performanceMissingCash",
  "insufficient-data": "performanceMissingData",
  "load-failed": "performanceLoadFailed",
  "non-positive-capital": "performanceNonPositiveCapital",
  unavailable: "performanceUnavailable",
} as const satisfies Record<PerformanceLine["status"], PortfolioMessageKey>;

const Rate = (props: {
  readonly status: PerformanceLine["status"];
  readonly value: SignedRate | undefined;
}): ReactElement =>
  props.value === undefined ? (
    <Text variant="caption" tone="ink-muted">
      {portfolioText(statusKeys[props.status], "ko")}
    </Text>
  ) : (
    <Percent value={props.value.rate} direction={props.value.direction} />
  );

const MetricRow = (props: {
  readonly label: UiText;
  readonly children: ReactElement;
}): ReactElement => (
  <TableRow>
    <TableRowHeader>{props.label}</TableRowHeader>
    <TableCell align="end">{props.children}</TableCell>
  </TableRow>
);

const PeriodRows = (props: {
  readonly lines: readonly PerformanceLine[];
}): ReactElement => {
  const year = props.lines.find((line) => line.span === "1y");
  const first = props.lines.at(0);
  if (
    first !== undefined &&
    year?.maxDrawdown === undefined &&
    props.lines.every(
      (line) => line.periodReturn === undefined && line.status === first.status,
    )
  )
    return (
      <MetricRow label={portfolioText("performanceCombined", "ko")}>
        <Rate status={first.status} value={undefined} />
      </MetricRow>
    );
  return (
    <>
      {props.lines.map((line) => (
        <MetricRow
          key={line.span}
          label={portfolioText(spanKeys[line.span], "ko")}
        >
          <Rate value={line.periodReturn} status={line.status} />
        </MetricRow>
      ))}
      <MetricRow label={portfolioText("performanceDrawdown", "ko")}>
        <Rate
          value={year?.maxDrawdown}
          status={year?.status ?? "unavailable"}
        />
      </MetricRow>
    </>
  );
};

export const PerformanceSummary = (props: {
  readonly today?: PortfolioView["today"];
}): ReactElement => {
  const query = useQuery({
    queryKey: [...queryKeys.portfolio, "performance"],
    queryFn: getPerformance,
    refetchInterval: (current) => {
      const reply = current.state.data;
      if (reply?.kind !== "ok") return refreshMs;
      if (reply.value.lines.some((line) => line.status === "calculating"))
        return 2_000;
      if (reply.value.lines.some((line) => line.status === "waiting"))
        return 10_000;
      return refreshMs;
    },
  });
  const reply = query.data;
  return (
    <TableRoot
      frame="embedded"
      dense="compact"
      caption={portfolioText("performanceMetrics", "ko")}
    >
      <TableBody>
        <MetricRow label={portfolioText("todayTitle", "ko")}>
          {props.today === undefined ? (
            <Text variant="caption" tone="ink-muted">
              {portfolioText("todayPending", "ko")}
            </Text>
          ) : (
            <PriceChange
              amount={props.today.amount}
              rate={props.today.rate}
              direction={props.today.direction}
            />
          )}
        </MetricRow>
        {reply === undefined || reply.kind === "err" ? (
          <MetricRow label={portfolioText("performanceTitle", "ko")}>
            <Text variant="caption" tone="ink-muted">
              {portfolioText(
                reply === undefined ? "performanceLoading" : "performanceError",
                "ko",
              )}
            </Text>
          </MetricRow>
        ) : (
          <PeriodRows lines={reply.value.lines} />
        )}
      </TableBody>
    </TableRoot>
  );
};
