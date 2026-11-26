import { useMutation, useQueryClient } from "@tanstack/react-query";
import type { ReactElement } from "react";
import { Stack, Switch, Text } from "../../../shared/ds";
import { setAutoStart, type ToggleState } from "../../../shared/ipc";
import { queryKeys } from "../../../shared/lib";
import { detach } from "../../../shared/platform";
import { settingsText } from "../messages";

export const AutoStartSwitch = (props: {
  readonly state: ToggleState;
}): ReactElement => {
  const client = useQueryClient();
  const change = useMutation({
    mutationFn: setAutoStart,
    onSettled: (): void => {
      detach({
        kind: "invalidate-queries",
        client,
        keys: [queryKeys.settings],
      });
    },
  });
  const failed = change.data?.kind === "err";
  return (
    <Stack gap="space-1">
      {[
        <Switch
          key="switch"
          label={settingsText("autoStartLabel", "ko")}
          description={settingsText("autoStartHint", "ko")}
          checked={props.state}
          disabled={change.isPending ? "disabled" : "enabled"}
          onChange={(state): void => {
            change.mutate(state);
          }}
        />,
        ...(failed
          ? [
              <Text key="failed" variant="caption" tone="ink-muted">
                {settingsText("autoStartFailed", "ko")}
              </Text>,
            ]
          : []),
      ]}
    </Stack>
  );
};
