import { useState, type ReactElement } from "react";
import { Button, Card, Stack, Text, TextField } from "../../../shared/ds";
import {
  parseSecretText,
  type NewsSettings,
  type OptionalKey,
  type RouteState,
} from "../../../shared/ipc";
import { emptyInputValue, type InputValue } from "../../../shared/lib";
import { newsText } from "../messages";
import { useKeySaver } from "../model";
import { useNewsClient } from "./client";
import { RouteLine } from "./RouteLine";

const KeyInput = (props: {
  readonly name: OptionalKey;
  readonly value: InputValue;
  readonly onChange: (value: InputValue) => void;
  readonly invalid: "valid" | "invalid";
}): ReactElement =>
  props.invalid === "invalid" ? (
    <TextField
      label={newsText(`news.key.${props.name}`, "ko")}
      type="secret"
      value={props.value}
      onChange={props.onChange}
      error={newsText("news.keyInvalid", "ko")}
    />
  ) : (
    <TextField
      label={newsText(`news.key.${props.name}`, "ko")}
      type="secret"
      value={props.value}
      onChange={props.onChange}
    />
  );

const KeyRow = (props: {
  readonly name: OptionalKey;
  readonly presence: "set" | "unset";
  readonly route?: RouteState;
}): ReactElement => {
  const saver = useKeySaver(useNewsClient());
  const [value, setValue] = useState<InputValue>(emptyInputValue);
  const [invalid, setInvalid] = useState<"valid" | "invalid">("valid");
  const submit = (): void => {
    const parsed = parseSecretText(value);
    setValue(emptyInputValue);
    if (parsed.kind === "err") {
      setInvalid("invalid");
      return;
    }
    setInvalid("valid");
    saver.save({ key: props.name, secret: parsed.value });
  };
  return (
    <Stack gap="space-2">
      <Text tone={props.presence === "set" ? "ink-muted" : "ink"}>
        {newsText(
          props.presence === "set" ? "news.keySet" : "news.keyUnset",
          "ko",
        )}
      </Text>
      {props.presence === "unset"
        ? [
            <Text key="benefit" variant="body-sm" tone="ink-muted">
              {newsText(`news.keyBenefit.${props.name}`, "ko")}
            </Text>,
          ]
        : []}
      {props.route === undefined
        ? []
        : [<RouteLine key="route" route="dart" state={props.route} />]}
      <KeyInput
        name={props.name}
        value={value}
        onChange={setValue}
        invalid={invalid}
      />
      <Button
        variant="secondary"
        disabled={saver.state === "pending" ? "disabled" : "enabled"}
        onClick={submit}
      >
        {newsText("news.save", "ko")}
      </Button>
      {saver.state === "failed"
        ? [
            <Text key="failed" tone="alert">
              {newsText("news.saveFailed", "ko")}
            </Text>,
          ]
        : []}
    </Stack>
  );
};

export const KeysCard = (props: {
  readonly keys: NewsSettings["keys"];
  readonly routes: NewsSettings["routes"];
}): ReactElement => (
  <Card title={newsText("news.keysTitle", "ko")}>
    <Stack gap="space-4">
      {props.routes === undefined ? (
        <KeyRow name="dart" presence={props.keys.dart} />
      ) : (
        <KeyRow
          name="dart"
          presence={props.keys.dart}
          route={props.routes.dart}
        />
      )}
      <KeyRow name="fred" presence={props.keys.fred} />
    </Stack>
  </Card>
);
