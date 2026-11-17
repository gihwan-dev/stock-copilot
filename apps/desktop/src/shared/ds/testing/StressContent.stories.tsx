import type { ReactElement } from "react";
import {
  emptyFreeText,
  parseChartDecimal,
  parseFreeText,
  parseMoney,
  parsePercentDisplay,
  parseQuantity,
} from "../../lib";
import { LineChart } from "../LineChart";
import { AreaChart } from "../AreaChart";
import { DonutChart } from "../DonutChart";
import { Treemap } from "../Treemap";
import {
  Money,
  Percent,
  Quantity,
  Probability,
  PriceChange,
  DirectionBadge,
} from "../Numbers";
import { Markdown } from "../Markdown";
import { Text } from "../Text";
import { chatInputStressWithDraft } from "../ChatInput";
import { stressFixtures } from "./stress";

type ContentStory = {
  readonly component:
    | "LineChart"
    | "AreaChart"
    | "DonutChart"
    | "Treemap"
    | "Money"
    | "Percent"
    | "Quantity"
    | "Probability"
    | "PriceChange"
    | "DirectionBadge"
    | "Markdown"
    | "ChatInput";
  readonly render: (theme?: "light" | "dark") => ReactElement | null;
};

const label = stressFixtures.company;
const money = parseMoney(stressFixtures.money, "KRW");
const quantity = parseQuantity(stressFixtures.quantity);
const percent = parsePercentDisplay(stressFixtures.money);
const probability = parsePercentDisplay(stressFixtures.probability);
const chartValue = parseChartDecimal(stressFixtures.money);
const points =
  chartValue.kind === "ok"
    ? stressFixtures.holdings
        .slice(0, 2)
        .map((id) => ({ id, label, value: chartValue.value }))
    : [];
const markdown = parseFreeText(
  [
    ...stressFixtures.paragraphs,
    ...stressFixtures.sources.map(
      (source) => `[${stressFixtures.english}](${source})`,
    ),
  ].join("\n\n"),
);
const chatDraft = parseFreeText(
  [label, ...stressFixtures.paragraphs].join("\n"),
);
const chatStress = chatInputStressWithDraft(
  chatDraft.kind === "ok" ? chatDraft.value : emptyFreeText,
);
const onSend = (): void => undefined;

export const contentStressStories: readonly ContentStory[] = [
  ...(["LineChart", "AreaChart", "DonutChart", "Treemap"] as const).map(
    (component): ContentStory => {
      const Chart = { LineChart, AreaChart, DonutChart, Treemap }[component];
      return {
        component,
        render: (theme = "light"): ReactElement => (
          <Chart
            data={points}
            label={label}
            empty={stressFixtures.empty}
            theme={theme}
          />
        ),
      };
    },
  ),
  {
    component: "Money",
    render: (): ReactElement | null =>
      money.kind === "ok" ? <Money value={money.value} /> : null,
  },
  {
    component: "Percent",
    render: (): ReactElement | null =>
      percent.kind === "ok" ? <Percent value={percent.value} /> : null,
  },
  {
    component: "Quantity",
    render: (): ReactElement | null =>
      quantity.kind === "ok" ? <Quantity value={quantity.value} /> : null,
  },
  {
    component: "Probability",
    render: (): ReactElement | null =>
      probability.kind === "ok" ? (
        <Probability value={probability.value} />
      ) : null,
  },
  {
    component: "PriceChange",
    render: (): ReactElement | null =>
      money.kind === "ok" && percent.kind === "ok" ? (
        <PriceChange amount={money.value} rate={percent.value} direction="up" />
      ) : null,
  },
  {
    component: "DirectionBadge",
    render: (): ReactElement => (
      <>
        <DirectionBadge direction="down" />
        <Text>{label}</Text>
      </>
    ),
  },
  {
    component: "Markdown",
    render: (): ReactElement => (
      <Markdown
        content={markdown.kind === "ok" ? markdown.value : emptyFreeText}
        linkAllowed={(): "deny" => "deny"}
        onOpenLink={onSend}
      />
    ),
  },
  ...chatStress,
];
