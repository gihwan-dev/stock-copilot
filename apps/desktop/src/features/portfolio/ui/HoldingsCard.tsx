import type { ReactElement } from "react";
import {
  Card,
  DataText,
  Money,
  PriceChange,
  Quantity,
  Stack,
  Table,
} from "../../../shared/ds";
import type { HoldingRow } from "../../../shared/ipc";
import type { EntityId, UiText } from "../../../shared/lib";
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
      <Stack gap="space-1">
        <DataText
          value={props.row.name}
          variant="emphasis"
          alert={
            props.row.trading === "suspended"
              ? portfolioText("tradingSuspended", "ko")
              : undefined
          }
          onPress={(): void => {
            props.open(props.row.id);
          }}
        />
        <DataText value={props.row.ticker} variant="code" />
      </Stack>
    </TableRowHeader>
    <TableCell align="end">
      <Quantity value={props.row.quantity} />
    </TableCell>
    <TableCell align="end">
      <Money value={props.row.value} emphasis="value" />
    </TableCell>
    <TableCell align="end">
      <PriceChange
        layout="stacked"
        amount={props.row.profit.amount}
        rate={props.row.profit.rate}
        direction={props.row.profit.direction}
      />
    </TableCell>
  </TableRow>
);

type PositionsProps = {
  readonly holdings: readonly HoldingRow[];
  readonly open: (symbol: EntityId) => void;
};
const GroupRows = (
  props: PositionsProps & { readonly label?: UiText },
): ReactElement => (
  <TableBody group={props.label}>
    {props.holdings.map((row): ReactElement => (
      <Row key={row.id} open={props.open} row={row} />
    ))}
  </TableBody>
);
const PositionTables = (props: PositionsProps): ReactElement => {
  const won = props.holdings.filter((row) => row.value.currency === "KRW");
  const dollars = props.holdings.filter((row) => row.value.currency === "USD");
  return (
    <TableRoot frame="positions">
      <TableHead>
        <TableRow>
          <TableHeader>{portfolioText("nameColumn", "ko")}</TableHeader>
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

      {dollars.length === 0 ? (
        <GroupRows holdings={props.holdings} open={props.open} />
      ) : (
        <>
          {won.length === 0 ? null : (
            <GroupRows
              holdings={won}
              open={props.open}
              label={portfolioText("wonPositions", "ko")}
            />
          )}
          <GroupRows
            key="USD"
            holdings={dollars}
            open={props.open}
            label={portfolioText("dollarPositions", "ko")}
          />
        </>
      )}
    </TableRoot>
  );
};
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
      <TableRoot
        frame="embedded"
        empty={portfolioText("holdingsEmpty", "ko")}
      />
    ) : (
      <PositionTables holdings={props.holdings} open={props.onOpenInstrument} />
    )}
  </Card>
);
