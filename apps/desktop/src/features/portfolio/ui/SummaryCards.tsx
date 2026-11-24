import type { ReactElement } from "react";
import {
  Badge,
  Card,
  Grid,
  GridItem,
  Inline,
  Money,
  PriceChange,
  Stack,
  Table,
  Text,
  TimestampText,
} from "../../../shared/ds";
import type { CurrencyValue, PortfolioView } from "../../../shared/ipc";
import type { Currency } from "../../../shared/lib";
import { portfolioText, type PortfolioMessageKey } from "../messages";

const TableRoot = Table.Root;
const TableHead = Table.Head;
const TableBody = Table.Body;
const TableRow = Table.Row;
const TableHeader = Table.Header;
const TableRowHeader = Table.RowHeader;
const TableCell = Table.Cell;

const currencyKeys = {
  KRW: "currencyKrw",
  USD: "currencyUsd",
} as const satisfies Record<Currency, PortfolioMessageKey>;

const TotalCard = (props: { readonly view: PortfolioView }): ReactElement => (
  <Card title={portfolioText("totalTitle", "ko")}>
    <Stack gap="space-2">
      {[
        <Inline key="total" gap="space-2" align="center" wrap="wrap">
          <Money value={props.view.totalApproximate} notation="compact" />
          <Badge tone="neutral">
            {portfolioText("totalApproximate", "ko")}
          </Badge>
        </Inline>,
        <Text key="hint" variant="caption" tone="ink-muted">
          {portfolioText("totalHint", "ko")}
        </Text>,
        ...(props.view.exchangeRate === undefined
          ? []
          : [
              <Inline key="rate" gap="space-2" align="center" wrap="wrap">
                <Text variant="caption" tone="ink-muted">
                  {portfolioText("exchangeRateNote", "ko")}
                </Text>
                <TimestampText value={props.view.exchangeRate.at} />
              </Inline>,
            ]),
      ]}
    </Stack>
  </Card>
);

const TodayCard = (props: { readonly view: PortfolioView }): ReactElement => (
  <Card title={portfolioText("todayTitle", "ko")}>
    <Stack gap="space-2">
      <PriceChange
        amount={props.view.today.amount}
        rate={props.view.today.rate}
        direction={props.view.today.direction}
      />
      <Inline gap="space-2" align="center" wrap="wrap">
        <Text variant="caption" tone="ink-muted">
          {portfolioText("recordedAt", "ko")}
        </Text>
        <TimestampText value={props.view.recordedAt} />
      </Inline>
    </Stack>
  </Card>
);

const CurrencyRow = (props: {
  readonly value: CurrencyValue;
}): ReactElement => (
  <TableRow>
    <TableRowHeader>
      {portfolioText(currencyKeys[props.value.currency], "ko")}
    </TableRowHeader>
    <TableCell align="end">
      <Money value={props.value.holdings} />
    </TableCell>
    <TableCell align="end">
      <Money value={props.value.buyingPower} />
    </TableCell>
  </TableRow>
);

const CurrencyCard = (props: {
  readonly view: PortfolioView;
}): ReactElement => (
  <Card title={portfolioText("currencyTitle", "ko")}>
    <TableRoot dense="dense">
      <TableHead>
        <TableRow>
          <TableHeader>{portfolioText("currencyColumn", "ko")}</TableHeader>
          <TableHeader align="end">
            {portfolioText("holdingsColumn", "ko")}
          </TableHeader>
          <TableHeader align="end">
            {portfolioText("buyingPowerColumn", "ko")}
          </TableHeader>
        </TableRow>
      </TableHead>
      <TableBody>
        {props.view.currencies.map((value): ReactElement => (
          <CurrencyRow key={value.currency} value={value} />
        ))}
      </TableBody>
    </TableRoot>
  </Card>
);

export const SummaryCards = (props: {
  readonly view: PortfolioView;
}): ReactElement => (
  <Grid>
    <GridItem span={{ narrow: 12, medium: 6, wide: 4 }}>
      <TotalCard view={props.view} />
    </GridItem>
    <GridItem span={{ narrow: 12, medium: 6, wide: 4 }}>
      <TodayCard view={props.view} />
    </GridItem>
    <GridItem span={{ narrow: 12, medium: 12, wide: 4 }}>
      <CurrencyCard view={props.view} />
    </GridItem>
  </Grid>
);
