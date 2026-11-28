import type { ReactElement } from "react";
import {
  Badge,
  Card,
  DataText,
  Inline,
  Money,
  PriceChange,
  Quantity,
  Table,
} from "../../../shared/ds";
import type { HoldingRow } from "../../../shared/ipc";
import type { EntityId } from "../../../shared/lib";
import { portfolioText } from "../messages";

const TableRoot = Table.Root;
const TableHead = Table.Head;
const TableBody = Table.Body;
const TableRow = Table.Row;
const TableHeader = Table.Header;
const TableRowHeader = Table.RowHeader;
const TableCell = Table.Cell;

const Row = (props: {
  readonly open: (symbol: EntityId) => void;
  readonly row: HoldingRow;
}): ReactElement => (
  <TableRow>
    <TableRowHeader>
      {props.row.trading === "suspended" ? (
        <Inline gap="space-2" align="center" wrap="wrap">
          <DataText
            value={props.row.name}
            variant="body-sm"
            onPress={(): void => {
              props.open(props.row.id);
            }}
          />
          <Badge tone="alert">{portfolioText("tradingSuspended", "ko")}</Badge>
        </Inline>
      ) : (
        <DataText
          value={props.row.name}
          variant="body-sm"
          onPress={(): void => {
            props.open(props.row.id);
          }}
        />
      )}
    </TableRowHeader>
    <TableCell>
      <DataText value={props.row.ticker} variant="code" />
    </TableCell>
    <TableCell align="end">
      <Quantity value={props.row.quantity} />
    </TableCell>
    <TableCell align="end">
      <Money value={props.row.value} />
    </TableCell>
    <TableCell align="end">
      <PriceChange
        amount={props.row.profit.amount}
        rate={props.row.profit.rate}
        direction={props.row.profit.direction}
      />
    </TableCell>
  </TableRow>
);

export const HoldingsCard = (props: {
  readonly holdings: readonly HoldingRow[];
  readonly onOpenInstrument: (symbol: EntityId) => void;
  readonly onOpenTransactions: () => void;
}): ReactElement => (
  <Card
    title={portfolioText("holdingsTitle", "ko")}
    layout="flow"
    actions={{
      label: portfolioText("openTransactions", "ko"),
      onPress: props.onOpenTransactions,
    }}
  >
    {props.holdings.length === 0 ? (
      <TableRoot empty={portfolioText("holdingsEmpty", "ko")} />
    ) : (
      <TableRoot>
        <TableHead>
          <TableRow>
            <TableHeader>{portfolioText("nameColumn", "ko")}</TableHeader>
            <TableHeader>{portfolioText("tickerColumn", "ko")}</TableHeader>
            <TableHeader align="end">
              {portfolioText("quantityColumn", "ko")}
            </TableHeader>
            <TableHeader align="end">
              {portfolioText("valueColumn", "ko")}
            </TableHeader>
            <TableHeader align="end">
              {portfolioText("profitColumn", "ko")}
            </TableHeader>
          </TableRow>
        </TableHead>
        <TableBody>
          {props.holdings.map((row): ReactElement => (
            <Row key={row.id} open={props.onOpenInstrument} row={row} />
          ))}
        </TableBody>
      </TableRoot>
    )}
  </Card>
);
