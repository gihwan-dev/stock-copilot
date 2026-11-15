import type { ReactElement } from "react";
import { text, parseChartDecimal, parseEntityId } from "../lib";
import type { ChartPoint } from "./chart-types";
import { LineChart } from "./LineChart";
import { AreaChart } from "./AreaChart";
import { DonutChart } from "./DonutChart";
import { Treemap } from "./Treemap";
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
type Story = {
  readonly component: keyof typeof components;
  readonly state: "normal" | "empty" | "single" | "load" | "invalid";
  readonly themes: readonly ["light", "dark"];
  readonly render: () => ReactElement;
};
export const chartComponentStories: readonly Story[] = (
  ["LineChart", "AreaChart", "DonutChart", "Treemap"] as const
).flatMap((component) => {
  const Component = components[component];
  const data = {
    normal,
    empty: [],
    single: normal.slice(0, 1),
    load: loads[
      component === "LineChart" || component === "AreaChart" ? "trend" : "parts"
    ],
    invalid,
  } as const;
  return (["normal", "empty", "single", "load", "invalid"] as const).flatMap(
    (state) =>
      (["light", "dark"] as const).map((theme) => ({
        component,
        state,
        themes: ["light", "dark"] as const,
        render: (): ReactElement => (
          <Component
            data={data[state]}
            label={text("app.title", "ko")}
            empty={text("ds.tableEmpty", "ko")}
            theme={theme}
          />
        ),
      })),
  );
});
