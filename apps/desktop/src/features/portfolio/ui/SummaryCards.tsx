import type { ReactElement } from "react";
import {
  Badge,
  Button,
  Card,
  Split,
  Stack,
  Inline,
  Money,
  Table,
  Text,
  TimestampText,
} from "../../../shared/ds";
import type { CurrencyValue, PortfolioView } from "../../../shared/ipc";
import { useRevealDockWidget, type Currency } from "../../../shared/lib";
import { portfolioText, type PortfolioMessageKey } from "../messages";
import { PerformanceSummary } from "./PerformanceCard";

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

const MetaLine = (props: { readonly view: PortfolioView }): ReactElement => (
  <Inline wrap="metadata">
    {[
      <Inline key="recorded" gap="space-1">
        <Text variant="caption" tone="ink-muted">
          {portfolioText("recordedAt", "ko")}
        </Text>
        <TimestampText value={props.view.recordedAt} />
      </Inline>,
      ...(props.view.usdConversion === undefined
        ? []
        : [
            <Inline key="rate-number" gap="space-1">
              <Text variant="caption" tone="ink-muted">
                {portfolioText("dollarUnit", "ko")}
              </Text>
              <Money
                value={props.view.usdConversion.krwPerUsd}
                emphasis="caption"
              />
            </Inline>,
          ]),
      ...(props.view.exchangeRate === undefined
        ? []
        : [
            <Inline key="rate-at" gap="space-1">
              <Text variant="caption" tone="ink-muted">
                {portfolioText("exchangeRateNote", "ko")}
              </Text>
              <TimestampText value={props.view.exchangeRate.at} />
            </Inline>,
          ]),
    ]}
  </Inline>
);

const TotalValue = (props: { readonly view: PortfolioView }): ReactElement => (
  <>
    <Inline gap="space-2" align="center" wrap="wrap">
      <Money
        value={props.view.totalApproximate}
        notation="full"
        emphasis="display"
      />
      <Badge tone="neutral">{portfolioText("totalApproximate", "ko")}</Badge>
    </Inline>
    <PerformanceSummary today={props.view.today} />
  </>
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

const CurrencyTable = (props: {
  readonly view: PortfolioView;
}): ReactElement => (
  <TableRoot
    dense="compact"
    frame="currency"
    caption={portfolioText("currencyTitle", "ko")}
  >
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
      {[
        ...props.view.currencies.map((value): ReactElement => (
          <CurrencyRow key={value.currency} value={value} />
        )),
        ...(props.view.usdConversion === undefined
          ? []
          : [
              <TableRow key="converted">
                <TableRowHeader>
                  {portfolioText("dollarConverted", "ko")}
                </TableRowHeader>
                <TableCell align="end">
                  <Money value={props.view.usdConversion.holdingsKrw} />
                </TableCell>
                <TableCell align="end">
                  <Money value={props.view.usdConversion.buyingPowerKrw} />
                </TableCell>
              </TableRow>,
            ]),
      ]}
    </TableBody>
  </TableRoot>
);

const ValuationNotes = (props: {
  readonly view: PortfolioView;
}): ReactElement => {
  const openWidget = useRevealDockWidget();
  return (
    <>
      <Stack gap="space-1">
        {[
          <Text key="basis" variant="caption" tone="ink-muted">
            {portfolioText("totalHint", "ko")}
          </Text>,
          ...(props.view.totalCoverage === "partial"
            ? [
                <Text key="partial" variant="caption" tone="ink-muted">
                  {portfolioText("totalPartial", "ko")}
                </Text>,
              ]
            : []),
          <MetaLine key="observed" view={props.view} />,
        ]}
      </Stack>
      <Inline gap="space-1" justify="end">
        <Button
          variant="secondary"
          size="control-height-sm"
          onClick={(): void => {
            openWidget("allocation");
          }}
        >
          {portfolioText("allocationLink", "ko")}
        </Button>
      </Inline>
    </>
  );
};

export const SummaryCard = (props: {
  readonly view: PortfolioView;
}): ReactElement => (
  <Card title={portfolioText("totalTitle", "ko")}>
    <Split
      size="chat-width"
      secondaryFooter={<ValuationNotes view={props.view} />}
    >
      <TotalValue view={props.view} />
      <CurrencyTable view={props.view} />
    </Split>
  </Card>
);
