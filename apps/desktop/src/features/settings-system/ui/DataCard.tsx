import { useState, type ReactElement } from "react";
import { Button, Card, Dialog, Inline, Stack, Text } from "../../../shared/ds";
import type { UiText } from "../../../shared/lib";
import { systemText } from "../messages";
import { useDataActions, type ActionOutcome } from "../model";
import { useSystemClient } from "./client";

const outcomeText = (outcome: ActionOutcome): UiText | undefined => {
  switch (outcome) {
    case "idle":
    case "pending":
      return undefined;
    case "backed-up":
      return systemText("system.backedUp", "ko");
    case "exported":
      return systemText("system.exported", "ko");
    case "cancelled":
      return systemText("system.exportCancelled", "ko");
    case "deleted":
      return systemText("system.deleted", "ko");
    case "failed":
      return systemText("system.actionFailed", "ko");
  }
};

const DeleteDialog = (props: {
  readonly open: "open" | "closed";
  readonly close: () => void;
  readonly confirm: () => void;
}): ReactElement => (
  <Dialog
    open={props.open}
    title={systemText("system.deleteTitle", "ko")}
    placement="sheet"
    onClose={props.close}
    actions={{
      confirm: {
        label: systemText("system.deleteConfirm", "ko"),
        onPress: props.confirm,
      },
      cancel: {
        label: systemText("system.deleteCancel", "ko"),
        onPress: props.close,
      },
    }}
  >
    <Text>{systemText("system.deleteBody", "ko")}</Text>
  </Dialog>
);

export const DataCard = (): ReactElement => {
  const actions = useDataActions(useSystemClient());
  const [confirm, setConfirm] = useState<"open" | "closed">("closed");
  const busy = actions.outcome === "pending" ? "disabled" : "enabled";
  const sentence = outcomeText(actions.outcome);
  const close = (): void => {
    setConfirm("closed");
  };
  return (
    <Card title={systemText("system.dataTitle", "ko")}>
      <Stack gap="space-3">
        <Text>{systemText("system.dataBody", "ko")}</Text>
        <Inline gap="space-2" wrap="wrap">
          <Button variant="secondary" disabled={busy} onClick={actions.backup}>
            {systemText("system.backup", "ko")}
          </Button>
          <Button
            variant="secondary"
            disabled={busy}
            onClick={actions.exportData}
          >
            {systemText("system.export", "ko")}
          </Button>
          <Button
            variant="ghost"
            disabled={busy}
            onClick={(): void => {
              setConfirm("open");
            }}
          >
            {systemText("system.delete", "ko")}
          </Button>
        </Inline>
        {sentence === undefined
          ? []
          : [
              <Text key="outcome" tone="ink-muted">
                {sentence}
              </Text>,
            ]}
        <DeleteDialog
          open={confirm}
          close={close}
          confirm={(): void => {
            close();
            actions.deleteAll();
          }}
        />
      </Stack>
    </Card>
  );
};
