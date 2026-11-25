import { useState, type ReactElement } from "react";
import { StatusBar } from "../../../shared/ds";
import type { UpdateClient } from "../../../shared/ipc";
import { updateText } from "../messages";
import { useUpdateActions, useUpdateStatus } from "../model/updates";
import { RestartSheet } from "./RestartSheet";

export const UpdateBar = (props: {
  readonly client: UpdateClient;
}): ReactElement | null => {
  const view = useUpdateStatus(props.client);
  const actions = useUpdateActions(props.client);
  const [sheet, setSheet] = useState<"open" | "closed">("closed");
  if (view.status !== "ready" || view.data.state.kind !== "ready") return null;
  const ready = view.data.state;
  return (
    <>
      <StatusBar
        tone="info"
        action={{
          label: updateText("restart", "ko"),
          onPress: (): void => {
            setSheet("open");
          },
        }}
      >
        {updateText("readyBar", "ko")}
      </StatusBar>
      <RestartSheet
        open={sheet}
        version={ready.version}
        notes={ready.notes}
        busy={actions.busy}
        close={(): void => {
          setSheet("closed");
        }}
        restart={actions.restart}
      />
    </>
  );
};
