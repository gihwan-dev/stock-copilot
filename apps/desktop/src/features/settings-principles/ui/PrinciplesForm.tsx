import { useState, type ReactElement } from "react";
import {
  Button,
  Card,
  Select,
  Stack,
  Text,
  TextField,
} from "../../../shared/ds";
import {
  cashMinPercents,
  concentrationCounts,
  parsePrinciple,
  type CashMinPercent,
  type ConcentrationCount,
  type PrincipleText,
  type Principles,
} from "../../../shared/ipc";
import {
  emptyInputValue,
  parseInputValue,
  type InputValue,
  type UiText,
} from "../../../shared/lib";
import { principlesText } from "../messages";
import type { SaveState } from "../model";

type Choice = { readonly value: InputValue; readonly label: UiText };
type Literal = CashMinPercent | ConcentrationCount;

const inputOf = (raw: Literal | PrincipleText): InputValue => {
  const value = parseInputValue(raw);
  return value.kind === "ok" ? value.value : emptyInputValue;
};

const choice = (raw: Literal, label: UiText): readonly Choice[] => {
  const value = parseInputValue(raw);
  return value.kind === "ok" ? [{ value: value.value, label }] : [];
};

const cashChoices: readonly Choice[] = cashMinPercents.flatMap((percent) =>
  choice(percent, principlesText(`principles.cash.${percent}`, "ko")),
);

const countChoices: readonly Choice[] = concentrationCounts.flatMap((count) =>
  choice(count, principlesText(`principles.count.${count}`, "ko")),
);

const saveText = (state: SaveState): UiText | undefined => {
  switch (state) {
    case "idle":
    case "pending":
      return undefined;
    case "saved":
      return principlesText("principles.saved", "ko");
    case "failed":
      return principlesText("principles.saveFailed", "ko");
  }
};

const TextInput = (props: {
  readonly value: InputValue;
  readonly onChange: (value: InputValue) => void;
  readonly invalid: "valid" | "invalid";
}): ReactElement =>
  props.invalid === "invalid" ? (
    <TextField
      label={principlesText("principles.textLabel", "ko")}
      value={props.value}
      onChange={props.onChange}
      error={principlesText("principles.textInvalid", "ko")}
    />
  ) : (
    <TextField
      label={principlesText("principles.textLabel", "ko")}
      value={props.value}
      onChange={props.onChange}
      hint={principlesText("principles.textHint", "ko")}
    />
  );

const LimitSelects = (props: {
  readonly cash: CashMinPercent;
  readonly count: ConcentrationCount;
  readonly setCash: (cash: CashMinPercent) => void;
  readonly setCount: (count: ConcentrationCount) => void;
}): ReactElement => (
  <Stack gap="space-3">
    <Select
      label={principlesText("principles.cashLabel", "ko")}
      options={cashChoices}
      value={inputOf(props.cash)}
      onChange={(value): void => {
        const picked = cashMinPercents.find((item) => item === value);
        if (picked !== undefined) props.setCash(picked);
      }}
    />
    <Select
      label={principlesText("principles.concentrationLabel", "ko")}
      options={countChoices}
      value={inputOf(props.count)}
      onChange={(value): void => {
        const picked = concentrationCounts.find((item) => item === value);
        if (picked !== undefined) props.setCount(picked);
      }}
    />
  </Stack>
);

export const PrinciplesForm = (props: {
  readonly principles: Principles;
  readonly state: SaveState;
  readonly save: (principles: Principles) => void;
}): ReactElement => {
  const [text, setText] = useState(() => inputOf(props.principles.text));
  const [cash, setCash] = useState(props.principles.cash_min_percent);
  const [count, setCount] = useState(props.principles.concentration_min_count);
  const [invalid, setInvalid] = useState<"valid" | "invalid">("valid");
  const sentence = saveText(props.state);
  const submit = (): void => {
    const parsed = parsePrinciple(text);
    setInvalid(parsed.kind === "err" ? "invalid" : "valid");
    if (parsed.kind === "ok")
      props.save({
        text: parsed.value,
        cash_min_percent: cash,
        concentration_min_count: count,
      });
  };
  return (
    <Card>
      <Stack gap="space-3">
        <Text tone="ink-muted">{principlesText("principles.body", "ko")}</Text>
        <TextInput value={text} onChange={setText} invalid={invalid} />
        <LimitSelects
          cash={cash}
          count={count}
          setCash={setCash}
          setCount={setCount}
        />
        <Button
          variant="primary"
          disabled={props.state === "pending" ? "disabled" : "enabled"}
          onClick={submit}
        >
          {principlesText("principles.save", "ko")}
        </Button>
        {sentence === undefined
          ? []
          : [
              <Text
                key="saved"
                tone={props.state === "failed" ? "alert" : "ink-muted"}
              >
                {sentence}
              </Text>,
            ]}
      </Stack>
    </Card>
  );
};
