import type { ReactElement } from "react";
import { StatusBar } from "../../../shared/ds";
import type { ScreenSources } from "../../../shared/ipc";
import { selfCheckText } from "../messages";
import { useSelfCheckNotice } from "../model/self-check";

export const SelfCheckBar = (props: {
  readonly load: ScreenSources["selfCheck"];
  readonly onOpenSettings: () => void;
}): ReactElement | null => {
  const notice = useSelfCheckNotice(props.load);
  if (notice.kind === "hidden") return null;
  const sentence = selfCheckText(notice.problem, "ko");
  return notice.action === "open-settings" ? (
    <StatusBar
      tone="warning"
      action={{
        label: selfCheckText("openSettings", "ko"),
        onPress: props.onOpenSettings,
      }}
    >
      {sentence}
    </StatusBar>
  ) : (
    <StatusBar tone="warning">{sentence}</StatusBar>
  );
};
