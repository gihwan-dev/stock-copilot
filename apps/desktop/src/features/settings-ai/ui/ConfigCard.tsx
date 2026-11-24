import { useState, type ReactElement } from "react";
import { Button, Card, Stack, TextField } from "../../../shared/ds";
import { parseConfigPath, type AiSettings } from "../../../shared/ipc";
import {
  emptyInputValue,
  parseInputValue,
  type InputValue,
} from "../../../shared/lib";
import { aiText } from "../messages";
import type { AiChange } from "../model";

const initialPath = (settings: AiSettings): InputValue => {
  if (settings.claude_config_dir === null) return emptyInputValue;
  const value = parseInputValue(settings.claude_config_dir);
  return value.kind === "ok" ? value.value : emptyInputValue;
};

const PathField = (props: {
  readonly value: InputValue;
  readonly onChange: (value: InputValue) => void;
  readonly invalid: "valid" | "invalid";
}): ReactElement =>
  props.invalid === "invalid" ? (
    <TextField
      label={aiText("ai.configLabel", "ko")}
      value={props.value}
      onChange={props.onChange}
      error={aiText("ai.configInvalid", "ko")}
    />
  ) : (
    <TextField
      label={aiText("ai.configLabel", "ko")}
      value={props.value}
      onChange={props.onChange}
      hint={aiText("ai.configHint", "ko")}
    />
  );

export const ConfigCard = (props: {
  readonly settings: AiSettings;
  readonly save: (change: AiChange) => void;
}): ReactElement => {
  const [path, setPath] = useState(() => initialPath(props.settings));
  const [invalid, setInvalid] = useState<"valid" | "invalid">("valid");
  const submit = (): void => {
    if (path.trim() === "") {
      setInvalid("valid");
      props.save({ kind: "config", path: null });
      return;
    }
    const parsed = parseConfigPath(path);
    if (parsed.kind === "err") {
      setInvalid("invalid");
      return;
    }
    setInvalid("valid");
    props.save({ kind: "config", path: parsed.value });
  };
  return (
    <Card title={aiText("ai.configTitle", "ko")}>
      <Stack gap="space-3">
        <PathField value={path} onChange={setPath} invalid={invalid} />
        <Button variant="secondary" onClick={submit}>
          {aiText("ai.save", "ko")}
        </Button>
      </Stack>
    </Card>
  );
};
