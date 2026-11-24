import type { ReactElement } from "react";
import {
  parseChartDecimal,
  parseMoney,
  parsePercentDisplay,
  parsePrice,
  parseQuantity,
  parseTimestamp,
} from "../lib";
import {
  DataText,
  DirectionBadge,
  Money,
  Percent,
  PriceChange,
  Probability,
  Quantity,
  TimestampText,
} from "./Numbers";

type Story = {
  readonly component:
    | "Money"
    | "Percent"
    | "Quantity"
    | "Probability"
    | "PriceChange"
    | "DirectionBadge"
    | "DataText"
    | "TimestampText";
  readonly themes: readonly ["light", "dark"];
  readonly render: () => ReactElement | null;
};
const money = parseMoney("12345678901234567", "KRW");
const ratio = parsePercentDisplay("-0.00");
const quantity = parseQuantity("0.123456");
const probability = parsePercentDisplay("73");
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
    (direction): readonly Story[] => {
      const amounts = {
        up: "1234000",
        down: "-5678000",
        flat: "0.00",
      } as const;
      const rates = { up: "5.9", down: "-3.40", flat: "0.000" } as const;
      const amount = parseMoney(amounts[direction], "KRW");
      const rate = parsePercentDisplay(rates[direction]);
      return [
        {
          component: "Money",
          themes: ["light", "dark"],
          render: (): ReactElement | null =>
            amount.kind === "ok" ? (
              <Money value={amount.value} direction={direction} />
            ) : null,
        },
        {
          component: "Percent",
          themes: ["light", "dark"],
          render: (): ReactElement | null =>
            rate.kind === "ok" ? (
              <Percent value={rate.value} direction={direction} />
            ) : null,
        },
        {
          component: "PriceChange",
          themes: ["light", "dark"],
          render: (): ReactElement | null =>
            amount.kind === "ok" && rate.kind === "ok" ? (
              <PriceChange
                amount={amount.value}
                rate={rate.value}
                direction={direction}
              />
            ) : null,
        },
        {
          component: "DirectionBadge",
          themes: ["light", "dark"],
          render: (): ReactElement => <DirectionBadge direction={direction} />,
        },
      ];
    },
  ),
  ...(["0", "100", "64.5"] as const).map((raw): Story => {
    const value = parsePercentDisplay(raw);
    return {
      component: "Probability",
      themes: ["light", "dark"],
      render: (): ReactElement | null =>
        value.kind === "ok" ? <Probability value={value.value} /> : null,
    };
  }),
  {
    component: "Money",
    themes: ["light", "dark"],
    render: (): ReactElement | null => {
      const value = parsePrice("182.34", "USD");
      return value.kind === "ok" ? <Money value={value.value} /> : null;
    },
  },
  {
    component: "DataText",
    themes: ["light", "dark"],
    render: (): ReactElement | null => {
      const value = parseChartDecimal("42");
      return value.kind === "ok" ? <DataText value={value.value} /> : null;
    },
  },
  {
    component: "TimestampText",
    themes: ["light", "dark"],
    render: (): ReactElement | null => {
      const value = parseTimestamp("2026-11-24T15:30:00+09:00");
      return value.kind === "ok" ? <TimestampText value={value.value} /> : null;
    },
  },
];
