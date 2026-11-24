import type { ReactElement } from "react";
import {
  Button,
  Card,
  Page,
  Skeleton,
  Stack,
  Text,
  TextField,
} from "../../../shared/ds";
import type { PublicIp } from "../../../shared/ipc";
import {
  parseInputValue,
  type InputValue,
  type UiText,
} from "../../../shared/lib";
import { onboardingText } from "../messages";
import { useIpCopy, usePublicIp, type CopyState } from "../model";
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

const IpField = (props: { readonly ip: PublicIp }): ReactElement => {
  const copier = useIpCopy(useOnboardingClient());
  const shown = parseInputValue(props.ip.address);
  if (shown.kind === "err")
    return <Text>{onboardingText("onboarding.ipUnavailable", "ko")}</Text>;
  return (
    <Stack gap="space-2">
      <IpText value={shown.value} hint={copyHint(copier.state)} />
      <Button
        variant="secondary"
        onClick={(): void => {
          copier.copy(props.ip);
        }}
      >
        {onboardingText("onboarding.copy", "ko")}
      </Button>
    </Stack>
  );
};

const IpBody = (): ReactElement => {
  const view = usePublicIp(useOnboardingClient());
  switch (view.status) {
    case "loading":
      return <Skeleton variant="text" />;
    case "unavailable":
      return (
        <Text tone="alert">
          {onboardingText("onboarding.ipUnavailable", "ko")}
        </Text>
      );
    case "ready":
      return <IpField ip={view.ip} />;
  }
};

export const IpScreen = (): ReactElement => (
  <Page title={onboardingText("onboarding.ipTitle", "ko")}>
    <Card>
      <Stack gap="space-4">
        <Text>{onboardingText("onboarding.ipBody", "ko")}</Text>
        <Text>{onboardingText("onboarding.ipLocation", "ko")}</Text>
        <IpBody />
        <Text tone="ink-muted" variant="body-sm">
          {onboardingText("onboarding.autoRetry", "ko")}
        </Text>
      </Stack>
    </Card>
  </Page>
);
