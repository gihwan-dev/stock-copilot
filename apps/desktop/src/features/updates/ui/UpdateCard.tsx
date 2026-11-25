import { useState, type ReactElement } from "react";
import {
  Button,
  DataText,
  Inline,
  Section,
  Stack,
  Text,
} from "../../../shared/ds";
import type { UpdateClient, UpdateState } from "../../../shared/ipc";
import type { UiText } from "../../../shared/lib";
import { updateText } from "../messages";
import { useUpdateActions, useUpdateStatus } from "../model/updates";
import { RestartSheet } from "./RestartSheet";

const failureText = {
  check: "stateCheckFailed",
  download: "stateDownloadFailed",
  install: "stateInstallFailed",
} as const;

const stateText = (state: UpdateState): UiText => {
  switch (state.kind) {
    case "off":
      return updateText("stateOff", "ko");
    case "idle":
      return updateText("stateIdle", "ko");
    case "checking":
      return updateText("stateChecking", "ko");
    case "downloading":
      return updateText("stateDownloading", "ko");
    case "up-to-date":
      return updateText("stateUpToDate", "ko");
    case "ready":
      return updateText("stateReady", "ko");
    case "failed":
      return updateText(failureText[state.failure], "ko");
  }
};

const ReadyAction = (props: {
  readonly state: UpdateState;
  readonly busy: "enabled" | "disabled";
  readonly restart: () => void;
}): ReactElement | null => {
  const [sheet, setSheet] = useState<"open" | "closed">("closed");
  if (props.state.kind !== "ready") return null;
  return (
    <>
      <Button
        onClick={(): void => {
          setSheet("open");
        }}
      >
        {updateText("restart", "ko")}
      </Button>
      <RestartSheet
        open={sheet}
        version={props.state.version}
        notes={props.state.notes}
        busy={props.busy}
        close={(): void => {
          setSheet("closed");
        }}
        restart={props.restart}
      />
    </>
  );
};

const checkAvailability = (
  state: UpdateState,
  busy: "enabled" | "disabled",
): "enabled" | "disabled" => {
  switch (state.kind) {
    case "off":
    case "checking":
    case "downloading":
    case "ready":
      return "disabled";
    case "idle":
    case "up-to-date":
    case "failed":
      return busy;
  }
};

export const UpdateCard = (props: {
  readonly client: UpdateClient;
}): ReactElement => {
  const view = useUpdateStatus(props.client);
  const actions = useUpdateActions(props.client);
  if (view.status !== "ready")
    return (
      <Section title={updateText("cardTitle", "ko")}>
        <Text tone="ink-muted">
          {view.status === "loading"
            ? updateText("stateLoading", "ko")
            : updateText("stateUnavailable", "ko")}
        </Text>
      </Section>
    );
  const state = view.data.state;
  return (
    <Section title={updateText("cardTitle", "ko")}>
      <Stack gap="space-3">
        <Inline gap="space-2">
          <Text tone="ink-muted">{updateText("currentVersion", "ko")}</Text>
          <DataText value={view.data.currentVersion} variant="code" />
        </Inline>
        <Text>{stateText(state)}</Text>
        <Inline gap="space-2" wrap="wrap">
          <Button
            variant="secondary"
            disabled={checkAvailability(state, actions.busy)}
            onClick={actions.check}
          >
            {updateText("check", "ko")}
          </Button>
          <ReadyAction
            state={state}
            busy={actions.busy}
            restart={actions.restart}
          />
        </Inline>
      </Stack>
    </Section>
  );
};
