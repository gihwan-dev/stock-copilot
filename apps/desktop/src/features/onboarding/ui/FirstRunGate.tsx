import { useState, type ReactElement } from "react";
import { Skeleton, StepScreen } from "../../../shared/ds";
import { onboardingText } from "../messages";
import { useConnectionGate } from "../model";
import { useOnboardingClient } from "./client";
import { IpScreen } from "./IpScreen";
import { KeyScreen } from "./KeyScreen";

export const FirstRunGate = (props: {
  readonly children: ReactElement;
}): ReactElement => {
  const view = useConnectionGate(useOnboardingClient());
  const [choice, setChoice] = useState<"follow" | "later">("follow");
  const later = (): void => {
    setChoice("later");
  };
  if (choice === "later" && view.screen !== "loading") return props.children;
  switch (view.screen) {
    case "loading":
      return (
        <StepScreen
          progress={["now", "next"]}
          progressLabel={onboardingText("onboarding.progress", "ko")}
          title={onboardingText("onboarding.loading", "ko")}
          description={onboardingText("onboarding.loadingBody", "ko")}
        >
          <Skeleton variant="text" />
        </StepScreen>
      );
    case "keys":
      return <KeyScreen problem={view.problem} onLater={later} />;
    case "ip":
      return <IpScreen onLater={later} />;
    case "ready":
      return props.children;
  }
};
