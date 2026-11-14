import type { ReactElement } from "react";
import { text } from "../lib";
import { Badge } from "./Badge";
import { StatusBar } from "./StatusBar";
type Story = {
  readonly component: "Badge" | "StatusBar";
  readonly state: "present" | "absent";
  readonly themes: readonly ["light", "dark"];
  readonly render: () => ReactElement;
};
const labels = {
  neutral: text("ds.badgeNeutral", "ko"),
  primary: text("ds.badgePrimary", "ko"),
  alert: text("ds.badgeAlert", "ko"),
} as const;
const sentences = {
  warning: text("ds.statusWarning", "ko"),
  error: text("ds.statusError", "ko"),
  info: text("ds.statusInfo", "ko"),
} as const;
export const statusComponentStories: readonly Story[] = [
  ...(["neutral", "primary", "alert"] as const).flatMap((tone) =>
    (["normal", "mono"] as const).map((mono): Story => ({
      component: "Badge",
      state: "present",
      themes: ["light", "dark"],
      render: (): ReactElement => (
        <Badge tone={tone} mono={mono}>
          {labels[tone]}
        </Badge>
      ),
    })),
  ),
  ...(["warning", "error", "info"] as const).flatMap((tone) =>
    (["present", "absent"] as const).map((action): Story => ({
      component: "StatusBar",
      state: "present",
      themes: ["light", "dark"],
      render: (): ReactElement =>
        action === "present" ? (
          <StatusBar
            tone={tone}
            action={{
              label: text("ds.retry", "ko"),
              onPress: (): void => undefined,
            }}
          >
            {sentences[tone]}
          </StatusBar>
        ) : (
          <StatusBar tone={tone}>{sentences[tone]}</StatusBar>
        ),
    })),
  ),
  ...(["warning", "error", "info"] as const).map((tone): Story => ({
    component: "StatusBar",
    state: "absent",
    themes: ["light", "dark"],
    render: (): ReactElement => (
      <StatusBar
        tone={tone}
        action={{
          label: text("ds.retry", "ko"),
          onPress: (): void => undefined,
        }}
      />
    ),
  })),
  {
    component: "StatusBar",
    state: "present",
    themes: ["light", "dark"],
    render: (): ReactElement => <StatusBar>{sentences.warning}</StatusBar>,
  },
];
