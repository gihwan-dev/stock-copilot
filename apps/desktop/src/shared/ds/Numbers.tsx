import type { ReactElement } from "react";
import { text } from "../lib";
import { Icon } from "./Icon";
import type {
  ChartDecimal,
  EntityId,
  FreeText,
  Money as MoneyValue,
  PercentDisplay,
  Price,
  Quantity as QuantityValue,
  Ticker,
  Timestamp,
  UiText,
} from "../lib";
import {
  formatNumber,
  formatPriceChange,
  formatDataValue,
  formatTimestampValue,
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
    className="tabular-nums whitespace-nowrap w-max text-alert"
  >
    {text("number.unavailable", "ko")}
  </span>
);
const Directed = (props: {
  readonly display: Extract<Display, { readonly kind: "available" }>;
  readonly direction: Direction;
}): ReactElement => (
  <span className="tabular-nums whitespace-nowrap w-max">
    <span className={colors[props.direction]}>
      <span role="img" aria-label={text(names[props.direction], "ko")}>
        {text(symbols[props.direction], "ko")}
      </span>
      {text("number.space", "ko")}
      {props.display.text}
    </span>
  </span>
);
type NumberEmphasis = "inherit" | "display" | "large";
const Sized = (props: {
  readonly emphasis: NumberEmphasis | undefined;
  readonly children: ReactElement;
}): ReactElement => {
  switch (props.emphasis) {
    case "display":
      return <span className="text-num-display">{props.children}</span>;
    case "large":
      return <span className="text-num-lg">{props.children}</span>;
    case "inherit":
    case undefined:
      return props.children;
  }
};
const Numeric = (props: {
  readonly display: Display;
  readonly direction: Direction | undefined;
}): ReactElement => {
  if (props.display.kind === "unavailable") return <Unavailable />;
  if (props.direction === undefined)
    return (
      <span className="tabular-nums whitespace-nowrap w-max">
        {props.display.text}
      </span>
    );
  return <Directed display={props.display} direction={props.direction} />;
};
export const Money = (props: {
  readonly value: MoneyValue | Price;
  readonly notation?: "full" | "compact";
  readonly direction?: Direction;
  readonly emphasis?: NumberEmphasis;
}): ReactElement => (
  <Sized emphasis={props.emphasis}>
    <Numeric
      display={formatNumber(
        {
          kind: "money",
          value: props.value,
          notation: props.notation ?? "full",
        },
        props.direction,
      )}
      direction={props.direction}
    />
  </Sized>
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
export const ChartNumber = (props: {
  readonly value: ChartDecimal;
}): ReactElement => (
  <Numeric
    display={formatNumber({ kind: "decimal", value: props.value })}
    direction={undefined}
  />
);
export const chartNumberText = (value: ChartDecimal): Display["text"] =>
  formatNumber({ kind: "decimal", value }).text;
export const chartLabelText: (value: UiText | FreeText) => Display["text"] =
  formatDataValue;
const dataClasses = {
  emphasis: "status-sentence text-title-sm font-semibold text-ink",
  body: "status-sentence text-body text-ink",
  "body-sm": "status-sentence text-body-sm text-ink-muted",
  caption: "status-sentence text-caption text-ink-subtle",
  code: "status-sentence text-code text-ink-muted",
} as const;
export const DataText = (props: {
  readonly onPress?: () => void;
  readonly value: FreeText | Ticker | EntityId | ChartDecimal;
  readonly variant?: "body" | "body-sm" | "caption" | "code" | "emphasis";
}): ReactElement => {
  const variant = props.variant ?? "body";
  if (props.onPress !== undefined)
    return (
      <button
        type="button"
        onClick={props.onPress}
        className="inline-flex items-center gap-space-1 max-w-full text-left text-ink-muted rounded-sm hover:text-primary focus-visible:outline-solid focus-visible:outline-focus"
      >
        <span className={dataClasses[variant]}>
          {formatDataValue(props.value)}
        </span>
        <Icon name="chevron-right" size="icon-sm" />
      </button>
    );
  return (
    <span className={dataClasses[variant]}>{formatDataValue(props.value)}</span>
  );
};
export const TimestampText = (props: {
  readonly value: Timestamp;
  readonly variant?: "body-sm" | "caption" | "code";
}): ReactElement => {
  const variant = props.variant ?? "caption";
  return (
    <span className={dataClasses[variant]}>
      {formatTimestampValue(props.value)}
    </span>
  );
};
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
  readonly emphasis?: NumberEmphasis;
  readonly layout?: "inline" | "stacked";
}): ReactElement => {
  if (props.layout === "stacked") {
    if (directionStatus(props.direction) === "invalid") return <Unavailable />;
    const amount = formatNumber(
      { kind: "money", value: props.amount },
      props.direction,
    );
    const rate = formatNumber(
      { kind: "percent", value: props.rate },
      props.direction,
    );
    if (amount.kind === "unavailable" || rate.kind === "unavailable")
      return <Unavailable />;
    return (
      <span
        role="group"
        aria-label={text(names[props.direction], "ko")}
        className="inline-flex flex-col items-end py-space-2 tabular-nums whitespace-nowrap"
      >
        <span className="text-num-md text-ink">{amount.text}</span>
        <span className={colors[props.direction]}>
          <span className="text-num-sm">{rate.text}</span>
        </span>
      </span>
    );
  }
  return (
    <Sized emphasis={props.emphasis}>
      <Numeric
        display={formatPriceChange(props.amount, props.rate, props.direction)}
        direction={props.direction}
      />
    </Sized>
  );
};
