import type { ReactElement } from "react";
import { Card, Page, Skeleton } from "../../../shared/ds";
import { onboardingText } from "../messages";
import { useConnectionGate } from "../model";
import { useOnboardingClient } from "./client";
import { IpScreen } from "./IpScreen";
import { KeyScreen } from "./KeyScreen";

export const FirstRunGate = (props: {
  readonly children: ReactElement;
}): ReactElement => {
  const view = useConnectionGate(useOnboardingClient());
  switch (view.screen) {
    case "loading":
      return (
        <Page title={onboardingText("onboarding.keyTitle", "ko")}>
          <Card title={onboardingText("onboarding.loading", "ko")}>
            <Skeleton variant="text" />
          </Card>
        </Page>
      );
    case "keys":
      return <KeyScreen problem={view.problem} />;
    case "ip":
      return <IpScreen />;
    case "ready":
      return props.children;
  }
};
