import { useQuery } from "@tanstack/react-query";
import type { ReactElement } from "react";
import { Inline, Percent, Stack, Text } from "../../../shared/ds";
import {
  getPerformance,
  type PerformanceLine,
  type PerformanceSpan,
  type SignedRate,
} from "../../../shared/ipc";
import { queryKeys } from "../../../shared/lib";
import { portfolioText, type PortfolioMessageKey } from "../messages";

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

const SpanRate = (props: { readonly line: PerformanceLine }): ReactElement => (
  <Inline gap="space-1" align="center">
    <Text variant="caption" tone="ink-muted">
      {portfolioText(spanKeys[props.line.span], "ko")}
    </Text>
    <Rate value={props.line.periodReturn} status={props.line.status} />
  </Inline>
);

const Lines = (props: {
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
      <Inline align="center" gap="space-2">
        <Text variant="caption">
          {portfolioText("performanceCombined", "ko")}
        </Text>
        <Rate status={first.status} value={undefined} />
      </Inline>
    );
  return (
    <Stack gap="space-1">
      <Inline gap="space-2" align="center" wrap="wrap">
        {[
          <Text key="title" variant="caption" tone="ink-muted">
            {portfolioText("performanceTitle", "ko")}
          </Text>,
          ...props.lines.map((line): ReactElement => (
            <SpanRate key={line.span} line={line} />
          )),
        ]}
      </Inline>
      <Inline key="drawdown" gap="space-1" align="center">
        <Text variant="caption" tone="ink-muted">
          {portfolioText("performanceDrawdown", "ko")}
        </Text>
        <Rate
          value={year?.maxDrawdown}
          status={year?.status ?? "unavailable"}
        />
      </Inline>
    </Stack>
  );
};

export const PerformanceSummary = (): ReactElement => {
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
  if (reply === undefined || reply.kind === "err")
    return (
      <Text variant="caption" tone="ink-muted">
        {portfolioText(
          reply === undefined ? "performanceLoading" : "performanceError",
          "ko",
        )}
      </Text>
    );
  return <Lines lines={reply.value.lines} />;
};
