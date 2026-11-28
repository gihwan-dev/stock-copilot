import type { ReactElement } from "react";
import { dataClasses, DataAction } from "./DataAction";
import { NumberSize, type NumberEmphasis } from "./NumberSize";
import { Icon } from "./Icon";
import { text } from "../lib";
import type {
  ChartDecimal,
  EntityId,
  Ticker,
  Timestamp,
  FreeText,
  Money as MoneyValue,
  PercentDisplay,
  Price,
  Quantity as QuantityValue,
  UiText,
} from "../lib";
import {
  formatNumber,
  formatPriceChange,
  formatDataValue,
  dataValueWords,
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
  <NumberSize emphasis={props.emphasis}>
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
  </NumberSize>
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
export const DataText = (props: {
  readonly onPress?: () => void;
  readonly alert?: UiText | undefined;
  readonly value: FreeText | Ticker | EntityId | ChartDecimal;
  readonly variant?: "body" | "body-sm" | "caption" | "code" | "emphasis";
}): ReactElement => {
  const variant = props.variant ?? "body";
  const value = formatDataValue(props.value);
  if (props.onPress === undefined)
    return <span className={dataClasses[variant]}>{value}</span>;
  const words = dataValueWords(value);
  return (
    <DataAction onPress={props.onPress} alert={props.alert}>
      <span className="status-sentence">
        {words.flatMap(({ offset, space, word }, i) => [
          space,
          i === words.length - 1 ? (
            <span
              key={offset}
              className="inline-flex max-w-full items-center gap-space-1"
            >
              <span>{word}</span>
              <Icon name="chevron-right" size="icon-sm" />
            </span>
          ) : (
            <span
              key={offset}
              className="inline-block max-w-full status-sentence"
            >
              {word}
            </span>
          ),
        ])}
      </span>
    </DataAction>
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
        className="inline-flex flex-col items-end gap-space-1 tabular-nums whitespace-nowrap"
      >
        <span className={colors[props.direction]}>
          <span className="text-body font-medium">{amount.text}</span>
        </span>
        <span className={colors[props.direction]}>
          <span className="text-num-sm">{rate.text}</span>
        </span>
      </span>
    );
  }
  return (
    <NumberSize emphasis={props.emphasis}>
      <Numeric
        display={formatPriceChange(props.amount, props.rate, props.direction)}
        direction={props.direction}
      />
    </NumberSize>
  );
};
