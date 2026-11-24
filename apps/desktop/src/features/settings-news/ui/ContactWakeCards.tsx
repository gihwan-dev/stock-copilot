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
  parseContact,
  type ContactText,
  type WakeCriteria,
} from "../../../shared/ipc";
import {
  emptyInputValue,
  parseInputValue,
  type InputValue,
} from "../../../shared/lib";
import { newsText } from "../messages";
import type { NewsChange } from "../model";
import {
  countChoices,
  inputOf,
  moveChoices,
  pickCount,
  pickMove,
} from "./choices";

type Save = (change: NewsChange) => void;

const initialContact = (contact: ContactText | null): InputValue => {
  if (contact === null) return emptyInputValue;
  const value = parseInputValue(contact);
  return value.kind === "ok" ? value.value : emptyInputValue;
};

const ContactField = (props: {
  readonly value: InputValue;
  readonly onChange: (value: InputValue) => void;
  readonly invalid: "valid" | "invalid";
}): ReactElement =>
  props.invalid === "invalid" ? (
    <TextField
      label={newsText("news.secLabel", "ko")}
      value={props.value}
      onChange={props.onChange}
      error={newsText("news.secInvalid", "ko")}
    />
  ) : (
    <TextField
      label={newsText("news.secLabel", "ko")}
      value={props.value}
      onChange={props.onChange}
      hint={newsText("news.secHint", "ko")}
    />
  );

export const ContactCard = (props: {
  readonly contact: ContactText | null;
  readonly save: Save;
}): ReactElement => {
  const [value, setValue] = useState(() => initialContact(props.contact));
  const [invalid, setInvalid] = useState<"valid" | "invalid">("valid");
  const submit = (): void => {
    if (value.trim() === "") {
      setInvalid("valid");
      props.save({ kind: "contact", contact: null });
      return;
    }
    const parsed = parseContact(value);
    if (parsed.kind === "err") {
      setInvalid("invalid");
      return;
    }
    setInvalid("valid");
    props.save({ kind: "contact", contact: parsed.value });
  };
  return (
    <Card title={newsText("news.secTitle", "ko")}>
      <Stack gap="space-3">
        <ContactField value={value} onChange={setValue} invalid={invalid} />
        <Button variant="secondary" onClick={submit}>
          {newsText("news.save", "ko")}
        </Button>
      </Stack>
    </Card>
  );
};

export const WakeCard = (props: {
  readonly wake: WakeCriteria;
  readonly save: Save;
}): ReactElement | null => {
  const move = inputOf(props.wake.daily_move_percent);
  const count = inputOf(props.wake.news_per_hour);
  if (move === undefined || count === undefined) return null;
  return (
    <Card title={newsText("news.wakeTitle", "ko")}>
      <Stack gap="space-3">
        <Text>{newsText("news.wakeDisclosure", "ko")}</Text>
        <Select
          label={newsText("news.wakeMove", "ko")}
          options={moveChoices}
          value={move}
          onChange={(value): void => {
            const picked = pickMove(value);
            if (picked !== undefined)
              props.save({
                kind: "wake",
                wake: { ...props.wake, daily_move_percent: picked },
              });
          }}
        />
        <Select
          label={newsText("news.wakeNews", "ko")}
          options={countChoices}
          value={count}
          onChange={(value): void => {
            const picked = pickCount(value);
            if (picked !== undefined)
              props.save({
                kind: "wake",
                wake: { ...props.wake, news_per_hour: picked },
              });
          }}
        />
      </Stack>
    </Card>
  );
};
