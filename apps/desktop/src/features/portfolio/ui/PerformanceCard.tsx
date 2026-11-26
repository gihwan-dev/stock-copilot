import { useQuery } from "@tanstack/react-query";
import type { ReactElement } from "react";
import { Inline, Percent, Text } from "../../../shared/ds";
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

const Rate = (props: {
  readonly value: SignedRate | undefined;
}): ReactElement =>
  props.value === undefined ? (
    <Text variant="caption" tone="ink-muted">
      {portfolioText("performancePending", "ko")}
    </Text>
  ) : (
    <Percent value={props.value.rate} direction={props.value.direction} />
  );

const SpanRate = (props: { readonly line: PerformanceLine }): ReactElement => (
  <Inline gap="space-1" align="center">
    <Text variant="caption" tone="ink-muted">
      {portfolioText(spanKeys[props.line.span], "ko")}
    </Text>
    <Rate value={props.line.periodReturn} />
  </Inline>
);

const Lines = (props: {
  readonly lines: readonly PerformanceLine[];
}): ReactElement => {
  const year = props.lines.find((line) => line.span === "1y");
  return (
    <Inline gap="space-2" align="center" wrap="wrap">
      {[
        <Text key="title" variant="caption" tone="ink-muted">
          {portfolioText("performanceTitle", "ko")}
        </Text>,
        ...props.lines.map((line): ReactElement => (
          <SpanRate key={line.span} line={line} />
        )),
        <Inline key="drawdown" gap="space-1" align="center">
          <Text variant="caption" tone="ink-muted">
            {portfolioText("performanceDrawdown", "ko")}
          </Text>
          <Rate value={year?.maxDrawdown} />
        </Inline>,
      ]}
    </Inline>
  );
};

export const PerformanceSummary = (): ReactElement => {
  const query = useQuery({
    queryKey: [...queryKeys.portfolio, "performance"],
    queryFn: getPerformance,
    refetchInterval: refreshMs,
  });
  const reply = query.data;
  if (reply === undefined || reply.kind === "err")
    return (
      <Text variant="caption" tone="ink-muted">
        {portfolioText(
          reply === undefined ? "performancePending" : "performanceError",
          "ko",
        )}
      </Text>
    );
  return <Lines lines={reply.value.lines} />;
};
