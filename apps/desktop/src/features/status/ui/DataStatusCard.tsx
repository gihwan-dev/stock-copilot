import type { ReactElement } from "react";
import {
  Card,
  Skeleton,
  Stack,
  Table,
  Text,
  TimestampText,
} from "../../../shared/ds";
import type {
  DataJobStatus,
  DataStatusView,
  IpcFailure,
} from "../../../shared/ipc";
import type { Result, Timestamp } from "../../../shared/lib";
import { statusText } from "../messages";
import { useDataStatus } from "../model/data-status";

const TableRoot = Table.Root;
const TableHead = Table.Head;
const TableBody = Table.Body;
const TableRow = Table.Row;
const TableHeader = Table.Header;
const TableRowHeader = Table.RowHeader;
const TableCell = Table.Cell;

const When = (props: {
  readonly at: Timestamp | undefined;
  readonly missing: "dataNever" | "dataNoSchedule";
}): ReactElement =>
  props.at === undefined ? (
    <Text variant="body-sm" tone="ink-muted">
      {statusText(props.missing, "ko")}
    </Text>
  ) : (
    <TimestampText value={props.at} variant="body-sm" />
  );

const JobRow = (props: { readonly row: DataJobStatus }): ReactElement => (
  <TableRow>
    <TableRowHeader>{statusText(`job.${props.row.job}`, "ko")}</TableRowHeader>
    <TableCell>
      <When at={props.row.lastSuccess} missing="dataNever" />
    </TableCell>
    <TableCell>
      <When at={props.row.nextRun} missing="dataNoSchedule" />
    </TableCell>
    <TableCell>
      {props.row.lastRun === undefined
        ? statusText("dataNever", "ko")
        : statusText(`run.${props.row.lastRun.outcome}`, "ko")}
    </TableCell>
  </TableRow>
);

const JobTable = (props: { readonly view: DataStatusView }): ReactElement =>
  props.view.jobs.length === 0 ? (
    <TableRoot empty={statusText("dataEmpty", "ko")} />
  ) : (
    <TableRoot dense="dense">
      <TableHead>
        <TableRow>
          <TableHeader>{statusText("dataJob", "ko")}</TableHeader>
          <TableHeader>{statusText("dataLastSuccess", "ko")}</TableHeader>
          <TableHeader>{statusText("dataNextRun", "ko")}</TableHeader>
          <TableHeader>{statusText("dataLastRun", "ko")}</TableHeader>
        </TableRow>
      </TableHead>
      <TableBody>
        {props.view.jobs.map((row): ReactElement => (
          <JobRow key={row.job} row={row} />
        ))}
      </TableBody>
    </TableRoot>
  );

const Body = (props: {
  readonly load: () => Promise<Result<DataStatusView, IpcFailure>>;
}): ReactElement => {
  const state = useDataStatus(props.load);
  switch (state.status) {
    case "loading":
      return <Skeleton variant="text" lines={3} />;
    case "error":
      return (
        <Text variant="body-sm" tone="ink-muted">
          {statusText("dataError", "ko")}
        </Text>
      );
    case "ready":
      return <JobTable view={state.data} />;
  }
};

export const DataStatusCard = (props: {
  readonly load: () => Promise<Result<DataStatusView, IpcFailure>>;
}): ReactElement => (
  <Card title={statusText("dataTitle", "ko")}>
    <Stack gap="space-3">
      <Text variant="caption" tone="ink-muted">
        {statusText("dataIntro", "ko")}
      </Text>
      <Body load={props.load} />
    </Stack>
  </Card>
);
