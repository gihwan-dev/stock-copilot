import type { ReactElement } from "react";
import { DataText, Dialog, Markdown, Stack, Text } from "../../../shared/ds";
import type { FreeText } from "../../../shared/lib";
import { updateText } from "../messages";

const noLinks = (): "deny" => "deny";
const ignoreLink = (): void => undefined;

export const RestartSheet = (props: {
  readonly open: "open" | "closed";
  readonly version: FreeText;
  readonly notes: FreeText;
  readonly busy: "enabled" | "disabled";
  readonly close: () => void;
  readonly restart: () => void;
}): ReactElement => (
  <Dialog
    open={props.open}
    title={updateText("sheetTitle", "ko")}
    placement="sheet"
    onClose={props.close}
    actions={{
      confirm: {
        label: updateText("sheetConfirm", "ko"),
        onPress: props.restart,
        disabled: props.busy,
      },
      cancel: {
        label: updateText("sheetLater", "ko"),
        onPress: props.close,
      },
    }}
  >
    <Stack gap="space-3">
      <Text>{updateText("sheetBody", "ko")}</Text>
      <Stack gap="space-1">
        <Text variant="label">{updateText("sheetVersion", "ko")}</Text>
        <DataText value={props.version} variant="code" />
      </Stack>
      {props.notes === ""
        ? []
        : [
            <Stack key="notes" gap="space-1">
              <Text variant="label">{updateText("sheetNotes", "ko")}</Text>
              <Markdown
                content={props.notes}
                linkAllowed={noLinks}
                onOpenLink={ignoreLink}
              />
            </Stack>,
          ]}
    </Stack>
  </Dialog>
);
