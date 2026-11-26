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

const TodayLine = (props: { readonly view: PortfolioView }): ReactElement => (
  <Inline gap="space-2" align="center" wrap="wrap">
    <Text variant="body-sm" tone="ink-muted">
      {portfolioText("todayTitle", "ko")}
    </Text>
    {props.view.today === undefined ? (
      <Text variant="body-sm" tone="ink-muted">
        {portfolioText("todayPending", "ko")}
      </Text>
    ) : (
      <PriceChange
        amount={props.view.today.amount}
        rate={props.view.today.rate}
        direction={props.view.today.direction}
        emphasis="large"
      />
    )}
  </Inline>
);

const MetaLine = (props: { readonly view: PortfolioView }): ReactElement => (
  <Inline gap="space-2" align="center" wrap="wrap">
    {[
      <Text key="recorded" variant="caption" tone="ink-muted">
        {portfolioText("recordedAt", "ko")}
      </Text>,
      <TimestampText key="recorded-at" value={props.view.recordedAt} />,
      ...(props.view.exchangeRate === undefined
        ? []
        : [
            <Text key="rate" variant="caption" tone="ink-muted">
              {portfolioText("exchangeRateNote", "ko")}
            </Text>,
            <TimestampText key="rate-at" value={props.view.exchangeRate.at} />,
          ]),
    ]}
  </Inline>
);

const TotalCard = (props: { readonly view: PortfolioView }): ReactElement => (
  <Card title={portfolioText("totalTitle", "ko")} layout="fill">
    <Stack gap="space-2">
      {[
        <Inline key="total" gap="space-2" align="center" wrap="wrap">
          <Money
            value={props.view.totalApproximate}
            notation="compact"
            emphasis="display"
          />
          <Badge tone="neutral">
            {portfolioText("totalApproximate", "ko")}
          </Badge>
        </Inline>,
        <TodayLine key="today" view={props.view} />,
        <MetaLine key="meta" view={props.view} />,
        <Text key="hint" variant="caption" tone="ink-muted">
          {portfolioText("totalHint", "ko")}
        </Text>,
        ...(props.view.totalCoverage === "partial"
          ? [
              <Text key="partial" variant="caption" tone="ink-muted">
                {portfolioText("totalPartial", "ko")}
              </Text>,
            ]
          : []),
      ]}
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
      {props.value.buyingPower === undefined ? (
        <Text variant="caption" tone="ink-muted">
          {portfolioText("notObserved", "ko")}
        </Text>
      ) : (
        <Money value={props.value.buyingPower} />
      )}
    </TableCell>
  </TableRow>
);

const CurrencyCard = (props: {
  readonly view: PortfolioView;
}): ReactElement => (
  <Card title={portfolioText("currencyTitle", "ko")} layout="fill">
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
  <Grid layout="fit-row">
    <GridItem span={6}>
      <TotalCard view={props.view} />
    </GridItem>
    <GridItem span={6}>
      <CurrencyCard view={props.view} />
    </GridItem>
  </Grid>
);
