import type { ReactElement } from "react";
import {
  Card,
  ChartNumber,
  DataText,
  Table,
  TimestampText,
} from "../../../shared/ds";
import type {
  AiUsageRow,
  CheckRecord,
  SourceStatus,
} from "../../../shared/ipc";
import { statusText } from "../messages";

const TableRoot = Table.Root;
const TableHead = Table.Head;
const TableBody = Table.Body;
const TableRow = Table.Row;
const TableHeader = Table.Header;
const TableRowHeader = Table.RowHeader;
const TableCell = Table.Cell;

export const RecordsCard = (props: {
  readonly records: readonly CheckRecord[];
}): ReactElement => (
  <Card title={statusText("recordsTitle", "ko")}>
    {props.records.length === 0 ? (
      <TableRoot empty={statusText("recordsEmpty", "ko")} />
    ) : (
      <TableRoot dense="dense">
        <TableHead>
          <TableRow>
            <TableHeader>{statusText("recordAt", "ko")}</TableHeader>
            <TableHeader>{statusText("recordCheck", "ko")}</TableHeader>
            <TableHeader>{statusText("recordDetail", "ko")}</TableHeader>
          </TableRow>
        </TableHead>
        <TableBody>
          {props.records.map((record): ReactElement => (
            <TableRow key={record.id}>
              <TableRowHeader>
                <TimestampText value={record.at} variant="body-sm" />
              </TableRowHeader>
              <TableCell>
                <DataText value={record.check} variant="body-sm" />
              </TableCell>
              <TableCell>
                <DataText value={record.detail} variant="body-sm" />
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </TableRoot>
    )}
  </Card>
);

export const SourcesCard = (props: {
  readonly sources: readonly SourceStatus[];
}): ReactElement => (
  <Card title={statusText("sourcesTitle", "ko")}>
    {props.sources.length === 0 ? (
      <TableRoot empty={statusText("sourcesEmpty", "ko")} />
    ) : (
      <TableRoot dense="dense">
        <TableHead>
          <TableRow>
            <TableHeader>{statusText("sourceName", "ko")}</TableHeader>
            <TableHeader>{statusText("sourceLastSuccess", "ko")}</TableHeader>
          </TableRow>
        </TableHead>
        <TableBody>
          {props.sources.map((source): ReactElement => (
            <TableRow key={source.id}>
              <TableRowHeader>
                <DataText value={source.name} variant="body-sm" />
              </TableRowHeader>
              <TableCell>
                {source.lastSuccess === undefined ? (
                  statusText("sourceNever", "ko")
                ) : (
                  <TimestampText value={source.lastSuccess} variant="body-sm" />
                )}
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </TableRoot>
    )}
  </Card>
);

export const UsageCard = (props: {
  readonly usage: readonly AiUsageRow[];
}): ReactElement => (
  <Card title={statusText("usageTitle", "ko")}>
    {props.usage.length === 0 ? (
      <TableRoot empty={statusText("usageEmpty", "ko")} />
    ) : (
      <TableRoot dense="dense">
        <TableHead>
          <TableRow>
            <TableHeader>{statusText("usageTask", "ko")}</TableHeader>
            <TableHeader align="end">
              {statusText("usageTokens", "ko")}
            </TableHeader>
            <TableHeader align="end">
              {statusText("usageSeconds", "ko")}
            </TableHeader>
          </TableRow>
        </TableHead>
        <TableBody>
          {props.usage.map((row): ReactElement => (
            <TableRow key={row.id}>
              <TableRowHeader>
                <DataText value={row.task} variant="body-sm" />
              </TableRowHeader>
              <TableCell align="end">
                <ChartNumber value={row.tokens} />
              </TableCell>
              <TableCell align="end">
                <ChartNumber value={row.seconds} />
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </TableRoot>
    )}
  </Card>
);
