import type { ReactElement } from "react";
import {
  DataText,
  Inline,
  LineChart,
  Percent,
  Stack,
  Table,
  Text,
  type ChartPoint,
} from "../../../shared/ds";
import type { ScorecardCombination } from "../../../shared/ipc";
import type { PercentDisplay, UiText } from "../../../shared/lib";
import { useColorScheme } from "../../../shared/platform";
import { scorecardText } from "../messages";

const ReliabilityChart = (props: {
  readonly data: readonly ChartPoint[];
}): ReactElement => (
  <LineChart
    data={props.data}
    label={scorecardText("scorecard.reliability", "ko")}
    empty={scorecardText("scorecard.reliabilityEmpty", "ko")}
    theme={useColorScheme()}
  />
);

const TableRoot = Table.Root;
const TableHead = Table.Head;
const TableBody = Table.Body;
const TableRow = Table.Row;
const TableHeader = Table.Header;
const TableRowHeader = Table.RowHeader;
const TableCell = Table.Cell;

const Line = (props: {
  readonly label: UiText;
  readonly children: ReactElement;
}): ReactElement => (
  <Inline gap="space-2" align="center" wrap="wrap">
    <Text variant="caption" tone="ink-muted">
      {props.label}
    </Text>
    {props.children}
  </Inline>
);

const MaybePercent = (props: {
  readonly value: PercentDisplay | undefined;
}): ReactElement =>
  props.value === undefined ? (
    <Text variant="body-sm">{scorecardText("scorecard.notYet", "ko")}</Text>
  ) : (
    <Percent value={props.value} />
  );

const Bins = (props: {
  readonly combination: ScorecardCombination;
}): ReactElement => (
  <TableRoot caption={scorecardText("scorecard.binsCaption", "ko")}>
    <TableHead>
      <TableRow>
        <TableHeader>{scorecardText("scorecard.bin", "ko")}</TableHeader>
        <TableHeader align="end">
          {scorecardText("scorecard.meanPrediction", "ko")}
        </TableHeader>
        <TableHeader align="end">
          {scorecardText("scorecard.hitRate", "ko")}
        </TableHeader>
        <TableHeader align="end">
          {scorecardText("scorecard.count", "ko")}
        </TableHeader>
      </TableRow>
    </TableHead>
    <TableBody>
      {props.combination.bins.map((bin): ReactElement => (
        <TableRow key={bin.id}>
          <TableRowHeader>
            <DataText value={bin.label} variant="body-sm" />
          </TableRowHeader>
          <TableCell align="end">
            <DataText value={bin.meanPrediction} variant="body-sm" />
          </TableCell>
          <TableCell align="end">
            <DataText value={bin.hitRate} variant="body-sm" />
          </TableCell>
          <TableCell align="end">
            <DataText value={bin.count} variant="body-sm" />
          </TableCell>
        </TableRow>
      ))}
    </TableBody>
  </TableRoot>
);

const Returns = (props: {
  readonly combination: ScorecardCombination;
}): ReactElement => (
  <Stack gap="space-1">
    <Line label={scorecardText("scorecard.signalReturn", "ko")}>
      <MaybePercent value={props.combination.signalReturn} />
    </Line>
    <Line label={scorecardText("scorecard.benchmark", "ko")}>
      <MaybePercent value={props.combination.benchmarkReturn} />
    </Line>
    <Line label={scorecardText("scorecard.interval", "ko")}>
      <Inline gap="space-1" align="center">
        <MaybePercent value={props.combination.intervalLow} />
        <Text variant="body-sm">
          {scorecardText("scorecard.intervalTo", "ko")}
        </Text>
        <MaybePercent value={props.combination.intervalHigh} />
      </Inline>
    </Line>
  </Stack>
);

export const CombinationView = (props: {
  readonly combination: ScorecardCombination;
}): ReactElement => {
  const points: readonly ChartPoint[] = props.combination.bins.map(
    (bin): ChartPoint => ({ id: bin.id, label: bin.label, value: bin.hitRate }),
  );
  return (
    <Stack gap="space-4">
      <Line label={scorecardText("scorecard.judged", "ko")}>
        <DataText value={props.combination.judged} variant="body-sm" />
      </Line>
      {props.combination.verdict === "pending" ? (
        <Text variant="body-sm" tone="alert">
          {scorecardText("scorecard.pending", "ko")}
        </Text>
      ) : (
        <Stack gap="space-1">{[]}</Stack>
      )}
      <Line label={scorecardText("scorecard.brier", "ko")}>
        {props.combination.brier === undefined ? (
          <Text variant="body-sm">
            {scorecardText("scorecard.notYet", "ko")}
          </Text>
        ) : (
          <DataText value={props.combination.brier} variant="body-sm" />
        )}
      </Line>
      <ReliabilityChart data={points} />
      <Bins combination={props.combination} />
      <Returns combination={props.combination} />
      <Line label={scorecardText("scorecard.comparisons", "ko")}>
        <DataText value={props.combination.comparisons} variant="body-sm" />
      </Line>
    </Stack>
  );
};
