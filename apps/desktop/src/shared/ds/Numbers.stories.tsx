import type { ReactElement } from "react";
import {
  parseMoney,
  parseRatio,
  parseQuantity,
  parseProbability,
} from "../lib";
import {
  Money,
  Percent,
  Quantity,
  Probability,
  PriceChange,
  DirectionBadge,
} from "./Numbers";

type Story = {
  readonly component:
    | "Money"
    | "Percent"
    | "Quantity"
    | "Probability"
    | "PriceChange"
    | "DirectionBadge";
  readonly themes: readonly ["light", "dark"];
  readonly render: () => ReactElement | null;
};
const money = parseMoney("12345678901234567.89", "KRW");
const ratio = parseRatio("-0.00");
const quantity = parseQuantity("0.123456");
const probability = parseProbability("73.2");
export const numberComponentStories: readonly Story[] = [
  ...(["full", "compact"] as const).map((notation): Story => ({
    component: "Money",
    themes: ["light", "dark"],
    render: (): ReactElement | null =>
      money.kind === "ok" ? (
        <Money value={money.value} notation={notation} />
      ) : null,
  })),
  {
    component: "Percent",
    themes: ["light", "dark"],
    render: (): ReactElement | null =>
      ratio.kind === "ok" ? <Percent value={ratio.value} /> : null,
  },
  {
    component: "Quantity",
    themes: ["light", "dark"],
    render: (): ReactElement | null =>
      quantity.kind === "ok" ? <Quantity value={quantity.value} /> : null,
  },
  {
    component: "Probability",
    themes: ["light", "dark"],
    render: (): ReactElement | null =>
      probability.kind === "ok" ? (
        <Probability value={probability.value} />
      ) : null,
  },
  ...(["up", "down", "flat"] as const).flatMap(
    (direction): readonly Story[] => [
      {
        component: "PriceChange",
        themes: ["light", "dark"],
        render: (): ReactElement | null =>
          money.kind === "ok" && ratio.kind === "ok" ? (
            <PriceChange
              amount={money.value}
              rate={ratio.value}
              direction={direction}
            />
          ) : null,
      },
      {
        component: "DirectionBadge",
        themes: ["light", "dark"],
        render: (): ReactElement => <DirectionBadge direction={direction} />,
      },
    ],
  ),
];
