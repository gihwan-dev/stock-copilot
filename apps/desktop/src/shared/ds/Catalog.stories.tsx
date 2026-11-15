import type { ReactElement } from "react";
import {
  text,
  parseMoney,
  parseEntityId,
  parseChartDecimal,
  type UiText,
} from "../lib";
import { Card } from "./Card";
import { Table } from "./Table";
import { Text } from "./Text";
import { Heading } from "./Heading";
import { EmptyState } from "./EmptyState";
import { Money } from "./Numbers";
import { LineChart } from "./LineChart";
import { AreaChart } from "./AreaChart";
import { DonutChart } from "./DonutChart";
import { Treemap } from "./Treemap";
import type { ChartPoint } from "./chart-types";
const amount = parseMoney("987654321098", "KRW");
const description = text("catalog.description", "ko");
const point = (
  id: "example-a" | "example-b" | "example-c",
  label: UiText,
  value: "24500000000" | "18600000000" | "32700000000",
): readonly ChartPoint[] => {
  const parsedId = parseEntityId(id);
  const parsedValue = parseChartDecimal(value);
  return parsedId.kind === "ok" && parsedValue.kind === "ok"
    ? [{ id: parsedId.value, label, value: parsedValue.value }]
    : [];
};
const trend = [
  ...point("example-a", text("catalog.firstDate", "ko"), "24500000000"),
  ...point("example-b", text("catalog.secondDate", "ko"), "18600000000"),
  ...point("example-c", text("catalog.thirdDate", "ko"), "32700000000"),
];
const parts = [
  ...point("example-a", text("catalog.company", "ko"), "24500000000"),
  ...point("example-b", text("catalog.companyTwo", "ko"), "18600000000"),
  ...point("example-c", text("catalog.companyThree", "ko"), "32700000000"),
];
const charts = { LineChart, AreaChart, DonutChart, Treemap } as const;
type Story = {
  readonly component:
    | "Text"
    | "Heading"
    | "Card"
    | "Table"
    | "EmptyState"
    | "LineChart"
    | "AreaChart"
    | "DonutChart"
    | "Treemap";
  readonly state: "long" | "empty" | "representative";
  readonly themes: readonly ["light", "dark"];
  readonly render: (theme?: "light" | "dark") => ReactElement;
};
export const catalogAdditionalStories: readonly Story[] = [
  {
    component: "Text",
    state: "long",
    themes: ["light", "dark"],
    render: (): ReactElement => <Text>{description}</Text>,
  },
  {
    component: "Heading",
    state: "long",
    themes: ["light", "dark"],
    render: (): ReactElement => (
      <Heading level={2}>{text("catalog.company", "ko")}</Heading>
    ),
  },
  {
    component: "Card",
    state: "long",
    themes: ["light", "dark"],
    render: (): ReactElement => (
      <Card title={text("catalog.holdings", "ko")}>
        <Text>{description}</Text>
        {amount.kind === "ok" ? (
          <Money value={amount.value} />
        ) : (
          <Text>{text("ds.tableEmpty", "ko")}</Text>
        )}
      </Card>
    ),
  },
  {
    component: "Table",
    state: "long",
    themes: ["light", "dark"],
    render: (): ReactElement => (
      <Table.Root caption={text("catalog.holdings", "ko")}>
        <Table.Head>
          <Table.Row>
            <Table.Header>{text("catalog.holdings", "ko")}</Table.Header>
            <Table.Header align="end">
              {text("catalog.amount", "ko")}
            </Table.Header>
          </Table.Row>
        </Table.Head>
        <Table.Body>
          <Table.Row>
            <Table.RowHeader>{text("catalog.company", "ko")}</Table.RowHeader>
            <Table.Cell align="end">
              {amount.kind === "ok" ? (
                <Money value={amount.value} />
              ) : (
                <Text>{text("ds.tableEmpty", "ko")}</Text>
              )}
            </Table.Cell>
          </Table.Row>
        </Table.Body>
      </Table.Root>
    ),
  },
  {
    component: "EmptyState",
    state: "empty",
    themes: ["light", "dark"],
    render: (): ReactElement => (
      <EmptyState title={text("ds.tableEmpty", "ko")}>{description}</EmptyState>
    ),
  },
  ...(["LineChart", "AreaChart", "DonutChart", "Treemap"] as const).map(
    (component): Story => {
      const Chart = charts[component];
      const kind =
        component === "LineChart" || component === "AreaChart"
          ? "trend"
          : "parts";
      return {
        component,
        state: "representative",
        themes: ["light", "dark"] as const,
        render: (theme: "light" | "dark" = "light"): ReactElement => (
          <Chart
            data={kind === "trend" ? trend : parts}
            label={text(
              kind === "trend" ? "catalog.trend" : "catalog.parts",
              "ko",
            )}
            empty={text("ds.tableEmpty", "ko")}
            theme={theme}
          />
        ),
      };
    },
  ),
];
