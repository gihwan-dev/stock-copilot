import { useState, type ReactElement } from "react";
import { Button, Card, Page, Stack, Text, TextField } from "../../../shared/ds";
import { parseSecretText, type TossKeys } from "../../../shared/ipc";
import {
  emptyInputValue,
  type InputValue,
  type UiText,
} from "../../../shared/lib";
import { onboardingText } from "../messages";
import { useKeySubmission, type KeyProblem } from "../model";
import { useOnboardingClient } from "./client";

const problemText = (problem: KeyProblem): UiText | undefined => {
  switch (problem) {
    case "none":
      return undefined;
    case "invalid-key":
      return onboardingText("onboarding.invalidKey", "ko");
    case "forbidden":
      return onboardingText("onboarding.forbidden", "ko");
    case "account-not-found":
      return onboardingText("onboarding.accountNotFound", "ko");
    case "key-in-use":
      return onboardingText("onboarding.keyInUse", "ko");
  }
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

type FieldCheck = "unchecked" | "invalid";

type KeyForm = {
  readonly clientId: InputValue;
  readonly secret: InputValue;
  readonly setClientId: (value: InputValue) => void;
  readonly setSecret: (value: InputValue) => void;
  readonly fieldError: UiText | undefined;
  readonly connect: () => void;
};

const useKeyForm = (submit: (keys: TossKeys) => void): KeyForm => {
  const [clientId, setClientId] = useState<InputValue>(emptyInputValue);
  const [secret, setSecret] = useState<InputValue>(emptyInputValue);
  const [check, setCheck] = useState<FieldCheck>("unchecked");
  return {
    clientId,
    secret,
    setClientId,
    setSecret,
    fieldError:
      check === "invalid"
        ? onboardingText("onboarding.invalidField", "ko")
        : undefined,
    connect: (): void => {
      const parsedId = parseSecretText(clientId);
      const parsedSecret = parseSecretText(secret);
      setClientId(emptyInputValue);
      setSecret(emptyInputValue);
      if (parsedId.kind === "err" || parsedSecret.kind === "err") {
        setCheck("invalid");
        return;
      }
      setCheck("unchecked");
      submit({ clientId: parsedId.value, secret: parsedSecret.value });
    },
  };
};

export const KeyScreen = (props: {
  readonly problem: KeyProblem;
}): ReactElement => {
  const submission = useKeySubmission(useOnboardingClient());
  const form = useKeyForm(submission.submit);
  const sentence =
    submission.state === "failed"
      ? onboardingText("onboarding.failed", "ko")
      : problemText(props.problem);
  return (
    <Page title={onboardingText("onboarding.keyTitle", "ko")}>
      <Card>
        <Stack gap="space-4">
          <Text>{onboardingText("onboarding.keyBody", "ko")}</Text>
          {sentence === undefined
            ? []
            : [
                <Text key="problem" tone="alert">
                  {sentence}
                </Text>,
              ]}
          <SecretField
            label={onboardingText("onboarding.clientIdLabel", "ko")}
            value={form.clientId}
            onChange={form.setClientId}
            error={form.fieldError}
          />
          <SecretField
            label={onboardingText("onboarding.secretLabel", "ko")}
            value={form.secret}
            onChange={form.setSecret}
            error={form.fieldError}
          />
          <Button
            variant="primary"
            disabled={submission.state === "pending" ? "disabled" : "enabled"}
            onClick={form.connect}
          >
            {onboardingText("onboarding.connect", "ko")}
          </Button>
        </Stack>
      </Card>
    </Page>
  );
};
