import type { ReactElement } from "react";
import { text, parseChartDecimal } from "../lib";
import type { ChartPoint } from "./chart-types";
import { LineChart } from "./LineChart";
import { AreaChart } from "./AreaChart";
import { DonutChart } from "./DonutChart";
import { Treemap } from "./Treemap";
const components = { LineChart, AreaChart, DonutChart, Treemap } as const;
const value = parseChartDecimal("12.125");
const other = parseChartDecimal("23.875");
const point: readonly ChartPoint[] =
  value.kind === "ok" && other.kind === "ok"
    ? [
        { label: text("ds.day", "ko"), value: value.value },
        { label: text("ds.month", "ko"), value: other.value },
      ]
    : [];
const load: readonly ChartPoint[] =
  value.kind === "ok"
    ? (
        [
          "ds.day",
          "ds.month",
          "ds.year",
          "ds.statusError",
          "ds.description",
          "app.title",
        ] as const
      ).map((key) => ({ label: text(key, "ko"), value: value.value }))
    : [];
const scenarios = {
  normal: point,
  empty: [],
  single: point.slice(0, 1),
  load,
  invalid: Array.from({ length: 501 }, () => point).flat(),
} as const;
type Story = {
  readonly component: keyof typeof components;
  readonly state: keyof typeof scenarios;
  readonly themes: readonly ["light", "dark"];
  readonly render: () => ReactElement;
};
export const chartComponentStories: readonly Story[] = (
  ["LineChart", "AreaChart", "DonutChart", "Treemap"] as const
).flatMap((component) =>
  (["normal", "empty", "single", "load", "invalid"] as const).flatMap((state) =>
    (["light", "dark"] as const).map((theme) => {
      const Component = components[component];
      return {
        component,
        state,
        themes: ["light", "dark"] as const,
        render: (): ReactElement => (
          <Component
            data={scenarios[state]}
            label={text("app.title", "ko")}
            empty={text("ds.tableEmpty", "ko")}
            theme={theme}
          />
        ),
      };
    }),
  ),
);
