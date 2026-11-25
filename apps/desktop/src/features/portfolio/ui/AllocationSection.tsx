import type { ReactElement } from "react";
import {
  Badge,
  Card,
  DataText,
  EmptyState,
  Grid,
  GridItem,
  Money,
  Percent,
  Section,
  Skeleton,
  Stack,
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
} as const satisfies Record<AllocationRow["level"], PortfolioMessageKey>;

const RowLabel = (props: { readonly row: AllocationRow }): ReactElement => {
  switch (props.row.kind) {
    case "cash-krw":
      return <Text variant="body-sm">{portfolioText("cashKrw", "ko")}</Text>;
    case "cash-usd":
      return <Text variant="body-sm">{portfolioText("cashUsd", "ko")}</Text>;
    case "holding":
    case undefined:
      return <DataText value={props.row.label} variant="body-sm" />;
  }
};

const Row = (props: { readonly row: AllocationRow }): ReactElement => (
  <TableRow>
    <TableRowHeader>
      <RowLabel row={props.row} />
    </TableRowHeader>
    <TableCell align="end">
      <Money value={props.row.value} notation="compact" />
    </TableCell>
    <TableCell align="end">
      <Percent value={props.row.weight} />
    </TableCell>
    <TableCell align="end">
      <Badge tone={props.row.level === "normal" ? "neutral" : "alert"}>
        {portfolioText(levelKeys[props.row.level], "ko")}
      </Badge>
    </TableCell>
  </TableRow>
);

const WeightTable = (props: {
  readonly titleKey: PortfolioMessageKey;
  readonly rows: readonly AllocationRow[];
}): ReactElement => (
  <Card title={portfolioText(props.titleKey, "ko")}>
    {props.rows.length === 0 ? (
      <TableRoot dense="dense" empty={portfolioText("allocationEmpty", "ko")} />
    ) : (
      <TableRoot dense="dense">
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
        <TableBody>
          {props.rows.map((row): ReactElement => (
            <Row key={row.id} row={row} />
          ))}
        </TableBody>
      </TableRoot>
    )}
  </Card>
);

const AllocationReady = (props: {
  readonly view: AllocationView;
}): ReactElement => (
  <Stack gap="space-4">
    <StatusBar
      tone={props.view.concentration === "concentrated" ? "warning" : "info"}
    >
      {portfolioText(props.view.concentration, "ko")}
    </StatusBar>
    <Grid>
      <GridItem span={{ narrow: 12, wide: 6 }}>
        <WeightTable titleKey="instrumentTitle" rows={props.view.instruments} />
      </GridItem>
      <GridItem span={{ narrow: 12, wide: 6 }}>
        <WeightTable titleKey="sectorTitle" rows={props.view.sectors} />
      </GridItem>
    </Grid>
  </Stack>
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
  <Section title={portfolioText("allocationSection", "ko")}>
    <AllocationBody load={props.load} retry={props.retry} />
  </Section>
);
