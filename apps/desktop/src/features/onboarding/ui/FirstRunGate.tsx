import { useMemo, useState, type ReactElement } from "react";
import { Skeleton, StepScreen } from "../../../shared/ds";
import { onboardingText } from "../messages";
import { useConnectionGate } from "../model";
import { useOnboardingClient } from "./client";
import { KeyEntryProvider, type KeyEntry } from "./entry";
import { IpScreen } from "./IpScreen";
import { KeyScreen } from "./KeyScreen";

type Choice = "follow" | "later" | "keys";

const Loading = (): ReactElement => (
  <StepScreen
    progress={["now", "next"]}
    progressLabel={onboardingText("onboarding.progress", "ko")}
    title={onboardingText("onboarding.loading", "ko")}
    description={onboardingText("onboarding.loadingBody", "ko")}
  >
    <Skeleton variant="text" />
  </StepScreen>
);

export const FirstRunGate = (props: {
  readonly children: ReactElement;
}): ReactElement => {
  const view = useConnectionGate(useOnboardingClient());
  const [choice, setChoice] = useState<Choice>("follow");
  const entry = useMemo<KeyEntry>(
    () => ({
      kind: "present",
      open: (): void => {
        setChoice("keys");
      },
    }),
    [],
  );
  const later = (): void => {
    setChoice("later");
  };
  const follow = (): void => {
    setChoice("follow");
  };
  const main = (
    <KeyEntryProvider entry={entry}>{props.children}</KeyEntryProvider>
  );
  if (view.screen === "loading") return <Loading />;
  if (choice === "keys")
    return <KeyScreen problem="none" onLater={later} onConnected={follow} />;
  if (choice === "later") return main;
  switch (view.screen) {
    case "keys":
      return (
        <KeyScreen
          problem={view.problem}
          onLater={later}
          onConnected={follow}
        />
      );
    case "ip":
      return <IpScreen onLater={later} />;
    case "ready":
      return main;
  }
};
