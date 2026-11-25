import type { ReactElement } from "react";
import { Skeleton, StepScreen, Text, TextField } from "../../../shared/ds";
import type { PublicIp } from "../../../shared/ipc";
import {
  parseInputValue,
  type InputValue,
  type UiText,
} from "../../../shared/lib";
import { onboardingText } from "../messages";
import { useIpCopy, usePublicIp, type CopyState, type IpView } from "../model";
import { useOnboardingClient } from "./client";

const copyHint = (state: CopyState): UiText | undefined => {
  switch (state) {
    case "idle":
      return undefined;
    case "copied":
      return onboardingText("onboarding.copied", "ko");
    case "failed":
      return onboardingText("onboarding.copyFailed", "ko");
  }
};

const IpText = (props: {
  readonly value: InputValue;
  readonly hint: UiText | undefined;
}): ReactElement =>
  props.hint === undefined ? (
    <TextField
      label={onboardingText("onboarding.ipLabel", "ko")}
      value={props.value}
      disabled="disabled"
    />
  ) : (
    <TextField
      label={onboardingText("onboarding.ipLabel", "ko")}
      value={props.value}
      disabled="disabled"
      hint={props.hint}
    />
  );

const IpField = (props: {
  readonly ip: PublicIp;
  readonly hint: UiText | undefined;
}): ReactElement => {
  const shown = parseInputValue(props.ip.address);
  if (shown.kind === "err")
    return <Text>{onboardingText("onboarding.ipUnavailable", "ko")}</Text>;
  return <IpText value={shown.value} hint={props.hint} />;
};

const IpBody = (props: {
  readonly view: IpView;
  readonly hint: UiText | undefined;
}): ReactElement => {
  switch (props.view.status) {
    case "loading":
      return <Skeleton variant="text" />;
    case "unavailable":
      return (
        <Text tone="alert">
          {onboardingText("onboarding.ipUnavailable", "ko")}
        </Text>
      );
    case "ready":
      return <IpField ip={props.view.ip} hint={props.hint} />;
  }
};

export const IpScreen = (props: {
  readonly onLater: () => void;
}): ReactElement => {
  const client = useOnboardingClient();
  const view = usePublicIp(client);
  const copier = useIpCopy(client);
  return (
    <StepScreen
      progress={["passed", "now"]}
      progressLabel={onboardingText("onboarding.progress", "ko")}
      title={onboardingText("onboarding.ipTitle", "ko")}
      description={onboardingText("onboarding.ipBody", "ko")}
      primary={{
        label: onboardingText("onboarding.copy", "ko"),
        onPress: (): void => {
          if (view.status === "ready") copier.copy(view.ip);
        },
        disabled: view.status === "ready" ? "enabled" : "disabled",
      }}
      secondary={{
        label: onboardingText("onboarding.later", "ko"),
        onPress: props.onLater,
      }}
    >
      <Text>{onboardingText("onboarding.ipLocation", "ko")}</Text>
      <IpBody view={view} hint={copyHint(copier.state)} />
      <Text tone="ink-muted" variant="body-sm">
        {onboardingText("onboarding.autoRetry", "ko")}
      </Text>
    </StepScreen>
  );
};
