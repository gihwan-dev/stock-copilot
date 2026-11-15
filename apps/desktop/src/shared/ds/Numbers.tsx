import type { ReactElement } from "react";
import { text } from "../lib";
import type {
  Money as MoneyValue,
  Price,
  Quantity as QuantityValue,
  PercentDisplay,
} from "../lib";
import {
  formatNumber,
  formatPriceChange,
  directionStatus,
  type Display,
} from "../lib/format";

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
const Unavailable = (): ReactElement => (
  <span
    role="img"
    aria-label={text("number.invalid", "ko")}
    className="tabular-nums whitespace-nowrap text-alert"
  >
    {text("number.unavailable", "ko")}
  </span>
);
const Directed = (props: {
  readonly display: Extract<Display, { readonly kind: "available" }>;
  readonly direction: Direction;
}): ReactElement => (
  <span className="tabular-nums whitespace-nowrap">
    <span className={colors[props.direction]}>
      <span role="img" aria-label={text(names[props.direction], "ko")}>
        {text(symbols[props.direction], "ko")}
      </span>
      {text("number.space", "ko")}
      {props.display.text}
    </span>
  </span>
);
const Numeric = (props: {
  readonly display: Display;
  readonly direction: Direction | undefined;
}): ReactElement => {
  if (props.display.kind === "unavailable") return <Unavailable />;
  if (props.direction === undefined)
    return (
      <span className="tabular-nums whitespace-nowrap">
        {props.display.text}
      </span>
    );
  return <Directed display={props.display} direction={props.direction} />;
};
export const Money = (props: {
  readonly value: MoneyValue | Price;
  readonly notation?: "full" | "compact";
  readonly direction?: Direction;
}): ReactElement => (
  <Numeric
    display={formatNumber(
      { kind: "money", value: props.value, notation: props.notation ?? "full" },
      props.direction,
    )}
    direction={props.direction}
  />
);
export const Percent = (props: {
  readonly value: PercentDisplay;
  readonly direction?: Direction;
}): ReactElement => (
  <Numeric
    display={formatNumber(
      { kind: "percent", value: props.value },
      props.direction,
    )}
    direction={props.direction}
  />
);
export const Quantity = (props: {
  readonly value: QuantityValue;
}): ReactElement => (
  <Numeric
    display={formatNumber({ kind: "quantity", value: props.value })}
    direction={undefined}
  />
);
export const Probability = (props: {
  readonly value: PercentDisplay;
}): ReactElement => (
  <Numeric
    display={formatNumber({ kind: "probability", value: props.value })}
    direction={undefined}
  />
);
export const DirectionBadge = (props: {
  readonly direction: Direction;
}): ReactElement => {
  if (directionStatus(props.direction) === "invalid") return <Unavailable />;
  return (
    <span className={colors[props.direction]}>
      <span aria-hidden="true">{text(symbols[props.direction], "ko")}</span>
      {text("number.space", "ko")}
      {text(names[props.direction], "ko")}
    </span>
  );
};
export const PriceChange = (props: {
  readonly amount: MoneyValue;
  readonly rate: PercentDisplay;
  readonly direction: Direction;
}): ReactElement => (
  <Numeric
    display={formatPriceChange(props.amount, props.rate, props.direction)}
    direction={props.direction}
  />
);
