import type { ReactElement } from "react";
import {
  Badge,
  Card,
  DataText,
  EmptyState,
  Money,
  Percent,
  Skeleton,
  StatusBar,
  Table,
  Text,
} from "../../../shared/ds";
import type {
  AllocationRow,
  AllocationView,
  ScreenSources,
} from "../../../shared/ipc";
import { portfolioText, type PortfolioMessageKey } from "../messages";
import { useAllocation } from "../model/portfolio";

const TableRoot = Table.Root;
const TableHead = Table.Head;
const TableBody = Table.Body;
const TableRow = Table.Row;
const TableHeader = Table.Header;
const TableRowHeader = Table.RowHeader;
const TableCell = Table.Cell;

const levelKeys = {
  normal: "levelNormal",
  warning: "levelWarning",
  "over-cap": "levelOverCap",
} as const satisfies Record<
  NonNullable<AllocationRow["level"]>,
  PortfolioMessageKey
>;

const allocationLabels = {
  "cash-krw": "cashKrw",
  "cash-usd": "cashUsd",
  "etf-fund": "sectorFund",
  cash: "sectorCash",
  unclassified: "sectorUnknown",
} as const;

const RowLabel = (props: { readonly row: AllocationRow }): ReactElement => {
  const kind = props.row.kind;
  if (kind === undefined || kind === "holding" || kind === "sector") {
    return <DataText value={props.row.label} variant="body-sm" />;
  }
  return (
    <Text variant="body-sm">{portfolioText(allocationLabels[kind], "ko")}</Text>
  );
};

const Row = (props: { readonly row: AllocationRow }): ReactElement => (
  <TableRow>
    <TableRowHeader>
      <RowLabel row={props.row} />
    </TableRowHeader>
    <TableCell align="end">
      <Money value={props.row.value} />
    </TableCell>
    <TableCell align="end">
      {props.row.weightRange === "below-unit" ? (
        <Text variant="body-sm">{portfolioText("weightBelowUnit", "ko")}</Text>
      ) : (
        <Percent value={props.row.weight} />
      )}
    </TableCell>
    <TableCell align="end">
      {props.row.level === null ? (
        <Text variant="body-sm">
          {portfolioText("levelNotApplicable", "ko")}
        </Text>
      ) : (
        <Badge tone={props.row.level === "normal" ? "neutral" : "alert"}>
          {portfolioText(levelKeys[props.row.level], "ko")}
        </Badge>
      )}
    </TableCell>
  </TableRow>
);

const WeightBodies = (props: {
  readonly rows: readonly AllocationRow[];
}): ReactElement => {
  const currencies = (["KRW", "USD"] as const)
    .map((currency) => ({
      currency,
      rows: props.rows.filter((row) => row.value.currency === currency),
    }))
    .filter((group) => group.rows.length > 0);
  return (
    <>
      {currencies.map(({ currency, rows }) => (
        <TableBody
          key={currency}
          group={
            currencies.length > 1
              ? portfolioText(
                  currency === "KRW" ? "allocationWon" : "allocationDollar",
                  "ko",
                )
              : undefined
          }
        >
          {rows.map((row) => (
            <Row key={row.id} row={row} />
          ))}
        </TableBody>
      ))}
    </>
  );
};

const WeightTable = (props: {
  readonly titleKey: PortfolioMessageKey;
  readonly rows: readonly AllocationRow[];
}): ReactElement =>
  props.rows.length === 0 ? (
    <TableRoot
      frame="embedded"
      dense="dense"
      caption={portfolioText(props.titleKey, "ko")}
      empty={portfolioText(
        props.titleKey === "sectorTitle" ? "sectorEmpty" : "allocationEmpty",
        "ko",
      )}
    />
  ) : (
    <TableRoot
      frame="embedded"
      dense="dense"
      caption={portfolioText(props.titleKey, "ko")}
    >
      <TableHead>
        <TableRow>
          <TableHeader>
            {portfolioText("allocationNameColumn", "ko")}
          </TableHeader>
          <TableHeader align="end">
            {portfolioText("valueColumn", "ko")}
          </TableHeader>
          <TableHeader align="end">
            {portfolioText("weightColumn", "ko")}
          </TableHeader>
          <TableHeader align="end">
            {portfolioText("levelColumn", "ko")}
          </TableHeader>
        </TableRow>
      </TableHead>
      <WeightBodies rows={props.rows} />
    </TableRoot>
  );

const AllocationReady = (props: {
  readonly view: AllocationView;
}): ReactElement => (
  <>
    {[
      <StatusBar
        key="concentration"
        tone={props.view.concentration === "concentrated" ? "warning" : "info"}
      >
        {portfolioText(props.view.concentration, "ko")}
      </StatusBar>,
      <WeightTable
        key="instruments"
        titleKey="instrumentTitle"
        rows={props.view.instruments}
      />,
      <Text key="sector-note" variant="body-sm">
        {portfolioText(
          props.view.sectorRefresh === "pending"
            ? "sectorPending"
            : "sectorBasis",
          "ko",
        )}
      </Text>,
      <WeightTable
        key="sectors"
        titleKey="sectorTitle"
        rows={props.view.sectors}
      />,
    ]}
  </>
);

const AllocationBody = (props: {
  readonly load: ScreenSources["allocation"];
  readonly retry: () => void;
}): ReactElement => {
  const view = useAllocation(props.load);
  switch (view.status) {
    case "loading":
      return <Skeleton variant="block" />;
    case "empty":
      return <EmptyState title={portfolioText("allocationEmpty", "ko")} />;
    case "error":
      return (
        <EmptyState
          title={portfolioText("allocationErrorTitle", "ko")}
          action={{ label: portfolioText("retry", "ko"), onPress: props.retry }}
        />
      );
    case "ready":
      return <AllocationReady view={view.data} />;
  }
};

export const AllocationSection = (props: {
  readonly load: ScreenSources["allocation"];
  readonly retry: () => void;
}): ReactElement => (
  <Card title={portfolioText("allocationSection", "ko")} layout="flow">
    <AllocationBody load={props.load} retry={props.retry} />
  </Card>
);
