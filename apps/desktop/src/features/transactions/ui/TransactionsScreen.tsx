import type { ReactElement } from "react";
import {
  Badge,
  Card,
  DataText,
  EmptyState,
  Inline,
  Money,
  Page,
  Quantity,
  Skeleton,
  Stack,
  Table,
  Text,
  TimestampText,
} from "../../../shared/ds";
import type {
  ScreenSources,
  TransactionKind,
  TransactionRow,
  TransactionsView,
} from "../../../shared/ipc";
import { transactionsText, type TransactionsMessageKey } from "../messages";
import { useTransactions } from "../model/transactions";

const TableRoot = Table.Root;
const TableHead = Table.Head;
const TableBody = Table.Body;
const TableRow = Table.Row;
const TableHeader = Table.Header;
const TableRowHeader = Table.RowHeader;
const TableCell = Table.Cell;

const kindKeys = {
  buy: "kindBuy",
  sell: "kindSell",
  dividend: "kindDividend",
  deposit: "kindDeposit",
  withdrawal: "kindWithdrawal",
  fee: "kindFee",
  other: "kindOther",
} as const satisfies Record<TransactionKind, TransactionsMessageKey>;

const Row = (props: { readonly row: TransactionRow }): ReactElement => (
  <TableRow>
    <TableRowHeader>
      <TimestampText value={props.row.settledAt} variant="body-sm" />
    </TableRowHeader>
    <TableCell>
      <DataText value={props.row.name} variant="body-sm" />
    </TableCell>
    <TableCell>{transactionsText(kindKeys[props.row.kind], "ko")}</TableCell>
    <TableCell align="end">
      {props.row.quantity === undefined ? (
        transactionsText("noQuantity", "ko")
      ) : (
        <Quantity value={props.row.quantity} />
      )}
    </TableCell>
    <TableCell align="end">
      <Money value={props.row.amount} />
    </TableCell>
    <TableCell>
      <Badge tone={props.row.presence === "vanished" ? "alert" : "neutral"}>
        {transactionsText(props.row.presence, "ko")}
      </Badge>
    </TableCell>
  </TableRow>
);

const TransactionsTable = (props: {
  readonly view: TransactionsView;
}): ReactElement => (
  <Card>
    <Stack gap="space-3">
      <Inline gap="space-2" align="center" wrap="wrap">
        <Text variant="caption" tone="ink-muted">
          {transactionsText("recordedAt", "ko")}
        </Text>
        <TimestampText value={props.view.recordedAt} />
      </Inline>
      <TableRoot caption={transactionsText("tableCaption", "ko")}>
        <TableHead>
          <TableRow>
            <TableHeader>{transactionsText("dateColumn", "ko")}</TableHeader>
            <TableHeader>{transactionsText("nameColumn", "ko")}</TableHeader>
            <TableHeader>{transactionsText("kindColumn", "ko")}</TableHeader>
            <TableHeader align="end">
              {transactionsText("quantityColumn", "ko")}
            </TableHeader>
            <TableHeader align="end">
              {transactionsText("amountColumn", "ko")}
            </TableHeader>
            <TableHeader>{transactionsText("stateColumn", "ko")}</TableHeader>
          </TableRow>
        </TableHead>
        <TableBody>
          {props.view.rows.map((row): ReactElement => (
            <Row key={row.id} row={row} />
          ))}
        </TableBody>
      </TableRoot>
      <Text variant="caption" tone="ink-muted">
        {transactionsText("vanishedNotice", "ko")}
      </Text>
    </Stack>
  </Card>
);

const TransactionsBody = (props: {
  readonly load: ScreenSources["transactions"];
}): ReactElement => {
  const transactions = useTransactions(props.load);
  switch (transactions.view.status) {
    case "loading":
      return (
        <Card title={transactionsText("loadingTitle", "ko")}>
          <Skeleton variant="text" lines={3} />
        </Card>
      );
    case "empty":
      return (
        <EmptyState title={transactionsText("emptyTitle", "ko")}>
          {transactionsText("emptyBody", "ko")}
        </EmptyState>
      );
    case "error":
      return (
        <EmptyState
          title={transactionsText("errorTitle", "ko")}
          action={{
            label: transactionsText("retry", "ko"),
            onPress: transactions.retry,
          }}
        />
      );
    case "ready":
      return <TransactionsTable view={transactions.view.data} />;
  }
};

export const TransactionsScreen = (props: {
  readonly load: ScreenSources["transactions"];
  readonly onBack: () => void;
}): ReactElement => (
  <Page
    title={transactionsText("title", "ko")}
    actions={{
      label: transactionsText("backToHome", "ko"),
      onPress: props.onBack,
    }}
  >
    <TransactionsBody load={props.load} />
  </Page>
);
