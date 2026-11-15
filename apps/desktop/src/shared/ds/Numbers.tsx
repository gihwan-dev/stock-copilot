import type { ReactElement } from "react";
import { formatNumber, text } from "../lib";
import type {
  Money as MoneyValue,
  Quantity as QuantityValue,
  Ratio,
  Probability as ProbabilityValue,
} from "../lib";

type Direction = "up" | "down" | "flat";
const colors = {
  up: "text-up",
  down: "text-down",
  flat: "text-ink-muted",
} as const;
const names = {
  up: "number.up",
  down: "number.down",
  flat: "number.flat",
} as const;
const symbols = {
  up: "number.upSymbol",
  down: "number.downSymbol",
  flat: "number.flatSymbol",
} as const;
export const Money = (props: {
  readonly value: MoneyValue;
  readonly notation?: "full" | "compact";
}): ReactElement => (
  <span className="tabular-nums whitespace-nowrap">
    {
      formatNumber({
        kind: "money",
        value: props.value,
        notation: props.notation ?? "full",
      }).text
    }
  </span>
);
export const Percent = (props: { readonly value: Ratio }): ReactElement => (
  <span className="tabular-nums whitespace-nowrap">
    {formatNumber({ kind: "percent", value: props.value }).text}
  </span>
);
export const Quantity = (props: {
  readonly value: QuantityValue;
}): ReactElement => (
  <span className="tabular-nums whitespace-nowrap">
    {formatNumber({ kind: "quantity", value: props.value }).text}
  </span>
);
export const Probability = (props: {
  readonly value: ProbabilityValue;
}): ReactElement => (
  <span className="tabular-nums whitespace-nowrap">
    {formatNumber({ kind: "probability", value: props.value }).text}
  </span>
);
export const DirectionBadge = (props: {
  readonly direction: Direction;
}): ReactElement => (
  <span className={colors[props.direction]}>
    <span aria-hidden="true">{text(symbols[props.direction], "ko")}</span>
    {text("number.space", "ko")}
    {text(names[props.direction], "ko")}
  </span>
);
export const PriceChange = (props: {
  readonly amount: MoneyValue;
  readonly rate: Ratio;
  readonly direction: Direction;
}): ReactElement => (
  <span className="tabular-nums whitespace-nowrap">
    <span className={colors[props.direction]}>
      <span role="img" aria-label={text(names[props.direction], "ko")}>
        {text(symbols[props.direction], "ko")}
      </span>
      {text("number.space", "ko")}
      {
        formatNumber({ kind: "money", value: props.amount }, props.direction)
          .text
      }
      {text("number.openRate", "ko")}
      {
        formatNumber({ kind: "percent", value: props.rate }, props.direction)
          .text
      }
      {text("number.closeRate", "ko")}
    </span>
  </span>
);
