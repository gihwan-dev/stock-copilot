import type { ReactElement } from "react";
import { text, parseChartDecimal, parseEntityId } from "../lib";
import type { CandlePoint, ChartPoint } from "./chart-types";
import { LineChart } from "./LineChart";
import { AreaChart } from "./AreaChart";
import { DonutChart } from "./DonutChart";
import { Treemap } from "./Treemap";
import { CandleChart } from "./CandleChart";
const components = { LineChart, AreaChart, DonutChart, Treemap } as const;
const point = (
  id: "first" | "second",
  value: "12.125" | "23.875",
): readonly ChartPoint[] => {
  const parsedId = parseEntityId(id);
  const parsedValue = parseChartDecimal(value);
  return parsedId.kind === "ok" && parsedValue.kind === "ok"
    ? [
        {
          id: parsedId.value,
          label: text(id === "first" ? "ds.day" : "ds.month", "ko"),
          value: parsedValue.value,
        },
      ]
    : [];
};
const normal = [...point("first", "12.125"), ...point("second", "23.875")];
const load = (size: 6 | 500 | 501): readonly ChartPoint[] =>
  Array.from(Array.from({ length: size }).keys(), (index) => {
    const id = parseEntityId(index.toString());
    const value = parseChartDecimal("12.125");
    return id.kind === "ok" && value.kind === "ok"
      ? [
          {
            id: id.value,
            label: text("ds.statusError", "ko"),
            value: value.value,
          },
        ]
      : [];
  }).flat();
const loads = { trend: load(500), parts: load(6) } as const;
const invalid = load(501);
const candlePoint = (
  id: "candle-1" | "candle-2",
  labelKey: "ds.day" | "ds.month",
  prices: readonly ["100" | "108", "112" | "110", "96" | "98", "108" | "101"],
): readonly CandlePoint[] => {
  const parsedId = parseEntityId(id);
  const open = parseChartDecimal(prices[0]);
  const high = parseChartDecimal(prices[1]);
  const low = parseChartDecimal(prices[2]);
  const close = parseChartDecimal(prices[3]);
  return parsedId.kind === "ok" &&
    open.kind === "ok" &&
    high.kind === "ok" &&
    low.kind === "ok" &&
    close.kind === "ok"
    ? [
        {
          id: parsedId.value,
          label: text(labelKey, "ko"),
          open: open.value,
          high: high.value,
          low: low.value,
          close: close.value,
        },
      ]
    : [];
};
const candleNormal = [
  ...candlePoint("candle-1", "ds.day", ["100", "112", "96", "108"]),
  ...candlePoint("candle-2", "ds.month", ["108", "110", "98", "101"]),
];
const candleLoad = (size: 500 | 501): readonly CandlePoint[] =>
  Array.from(Array.from({ length: size }).keys(), (index) => {
    const id = parseEntityId(index.toString());
    const price = parseChartDecimal("100");
    return id.kind === "ok" && price.kind === "ok"
      ? [
          {
            id: id.value,
            label: text("ds.statusError", "ko"),
            open: price.value,
            high: price.value,
            low: price.value,
            close: price.value,
          },
        ]
      : [];
  }).flat();
type Story = {
  readonly component: keyof typeof components | "CandleChart";
  readonly theme: "light" | "dark";
  readonly state: "normal" | "empty" | "single" | "load" | "invalid";
  readonly themes: readonly ["light", "dark"];
  readonly render: (theme?: "light" | "dark") => ReactElement;
};
const candleData = {
  normal: candleNormal,
  empty: [],
  single: candleNormal.slice(0, 1),
  load: candleLoad(500),
  invalid: candleLoad(501),
} as const;
const candleStories: readonly Story[] = (
  ["normal", "empty", "single", "load", "invalid"] as const
).flatMap((state) =>
  (["light", "dark"] as const).map((theme) => ({
    component: "CandleChart" as const,
    theme,
    state,
    themes: ["light", "dark"] as const,
    render: (catalogTheme = theme): ReactElement => (
      <CandleChart
        data={candleData[state]}
        label={text("app.title", "ko")}
        empty={text("ds.tableEmpty", "ko")}
        theme={catalogTheme}
      />
    ),
  })),
);
export const chartComponentStories: readonly Story[] = [
  ...(["LineChart", "AreaChart", "DonutChart", "Treemap"] as const).flatMap(
    (component) => {
      const Component = components[component];
      const data = {
        normal,
        empty: [],
        single: normal.slice(0, 1),
        load: loads[
          component === "LineChart" || component === "AreaChart"
            ? "trend"
            : "parts"
        ],
        invalid,
      } as const;
      return (
        ["normal", "empty", "single", "load", "invalid"] as const
      ).flatMap((state) =>
        (["light", "dark"] as const).map((theme) => ({
          component,
          theme,
          state,
          themes: ["light", "dark"] as const,
          render: (catalogTheme = theme): ReactElement => (
            <Component
              data={data[state]}
              label={text("app.title", "ko")}
              empty={text("ds.tableEmpty", "ko")}
              theme={catalogTheme}
            />
          ),
        })),
      );
    },
  ),
  ...candleStories,
];
