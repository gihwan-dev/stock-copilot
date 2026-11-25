import { useState, type ReactElement } from "react";
import { StepScreen, Text, TextField } from "../../../shared/ds";
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
  readonly onLater: () => void;
}): ReactElement => {
  const submission = useKeySubmission(useOnboardingClient());
  const form = useKeyForm(submission.submit);
  const sentence =
    submission.state === "failed"
      ? onboardingText("onboarding.failed", "ko")
      : problemText(props.problem);
  return (
    <StepScreen
      progress={["now", "next"]}
      progressLabel={onboardingText("onboarding.progress", "ko")}
      title={onboardingText("onboarding.keyTitle", "ko")}
      description={onboardingText("onboarding.keyBody", "ko")}
      primary={{
        label: onboardingText("onboarding.connect", "ko"),
        onPress: form.connect,
        disabled: submission.state === "pending" ? "disabled" : "enabled",
      }}
      secondary={{
        label: onboardingText("onboarding.later", "ko"),
        onPress: props.onLater,
      }}
    >
      {[
        ...(sentence === undefined
          ? []
          : [
              <Text key="problem" tone="alert">
                {sentence}
              </Text>,
            ]),
        <SecretField
          key="client-id"
          label={onboardingText("onboarding.clientIdLabel", "ko")}
          value={form.clientId}
          onChange={form.setClientId}
          error={form.fieldError}
        />,
        <SecretField
          key="secret"
          label={onboardingText("onboarding.secretLabel", "ko")}
          value={form.secret}
          onChange={form.setSecret}
          error={form.fieldError}
        />,
      ]}
    </StepScreen>
  );
};
