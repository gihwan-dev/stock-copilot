import { useState, type ReactElement } from "react";
import { Button, Card, Stack, Text, TextField } from "../../../shared/ds";
import { parseSecretText } from "../../../shared/ipc";
import {
  emptyInputValue,
  type InputValue,
  type UiText,
} from "../../../shared/lib";
import { basicText } from "../messages";
import { useKeyResubmit, useTossStatus, type TossStatus } from "../model";
import { useBasicClient } from "./client";

const statusLine = (status: TossStatus): ReactElement => {
  if (status === "loading")
    return <Text tone="ink-muted">{basicText("basic.loading", "ko")}</Text>;
  return (
    <Text tone={status === "connected" ? "ink" : "alert"}>
      {basicText(`basic.toss.${status}`, "ko")}
    </Text>
  );
};

const SecretField = (props: {
  readonly label: UiText;
  readonly value: InputValue;
  readonly onChange: (value: InputValue) => void;
  readonly error: UiText | undefined;
}): ReactElement =>
  props.error === undefined ? (
    <TextField
      label={props.label}
      type="secret"
      value={props.value}
      onChange={props.onChange}
    />
  ) : (
    <TextField
      label={props.label}
      type="secret"
      value={props.value}
      onChange={props.onChange}
      error={props.error}
    />
  );

export const TossCard = (): ReactElement => {
  const client = useBasicClient();
  const status = useTossStatus(client);
  const resubmit = useKeyResubmit(client);
  const [clientId, setClientId] = useState<InputValue>(emptyInputValue);
  const [secret, setSecret] = useState<InputValue>(emptyInputValue);
  const [invalid, setInvalid] = useState<"valid" | "invalid">("valid");
  const submit = (): void => {
    const parsedId = parseSecretText(clientId);
    const parsedSecret = parseSecretText(secret);
    setClientId(emptyInputValue);
    setSecret(emptyInputValue);
    if (parsedId.kind === "err" || parsedSecret.kind === "err") {
      setInvalid("invalid");
      return;
    }
    setInvalid("valid");
    resubmit.submit({ clientId: parsedId.value, secret: parsedSecret.value });
  };
  const fieldError =
    invalid === "invalid" ? basicText("basic.keyInvalid", "ko") : undefined;
  return (
    <Card title={basicText("basic.tossTitle", "ko")}>
      <Stack gap="space-3">
        {statusLine(status)}
        <SecretField
          label={basicText("basic.clientIdLabel", "ko")}
          value={clientId}
          onChange={setClientId}
          error={fieldError}
        />
        <SecretField
          label={basicText("basic.secretLabel", "ko")}
          value={secret}
          onChange={setSecret}
          error={undefined}
        />
        <Button
          variant="secondary"
          disabled={resubmit.state === "pending" ? "disabled" : "enabled"}
          onClick={submit}
        >
          {basicText("basic.reconnect", "ko")}
        </Button>
        {resubmit.state === "failed"
          ? [
              <Text key="failed" tone="alert">
                {basicText("basic.keyFailed", "ko")}
              </Text>,
            ]
          : []}
      </Stack>
    </Card>
  );
};
