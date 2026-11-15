import type { ReactElement } from "react";
import { text, parseFreeText, emptyFreeText } from "../lib";
import { Markdown } from "./Markdown";
import { ChatInput, chatInputStressStories } from "./ChatInput";
const example = parseFreeText(text("chat.example", "ko"));
const content = example.kind === "ok" ? example.value : emptyFreeText;
type Story = {
  readonly component: "Markdown" | "ChatInput";
  readonly state: "normal" | "denied" | "sending" | "stress";
  readonly themes: readonly ["light", "dark"];
  readonly render: () => ReactElement;
};
export const contentComponentStories: readonly Story[] = [
  {
    component: "Markdown",
    state: "normal",
    themes: ["light", "dark"],
    render: (): ReactElement => (
      <Markdown
        content={content}
        linkAllowed={(): "allow" => "allow"}
        onOpenLink={(): void => {
          return;
        }}
      />
    ),
  },
  {
    component: "Markdown",
    state: "denied",
    themes: ["light", "dark"],
    render: (): ReactElement => (
      <Markdown
        content={content}
        linkAllowed={(): "deny" => "deny"}
        onOpenLink={(): void => {
          return;
        }}
      />
    ),
  },
  {
    component: "Markdown",
    state: "stress",
    themes: ["light", "dark"],
    render: (): ReactElement => {
      const value = parseFreeText(
        (text("chat.example", "ko") + "\n\n").repeat(100),
      );
      return (
        <Markdown
          content={value.kind === "ok" ? value.value : emptyFreeText}
          linkAllowed={(url): "allow" | "deny" =>
            url.endsWith("/article") ? "allow" : "deny"
          }
          onOpenLink={(): void => {
            return;
          }}
        />
      );
    },
  },
  {
    component: "ChatInput",
    state: "normal",
    themes: ["light", "dark"],
    render: (): ReactElement => (
      <ChatInput
        onSend={(): void => {
          return;
        }}
      />
    ),
  },
  {
    component: "ChatInput",
    state: "sending",
    themes: ["light", "dark"],
    render: (): ReactElement => (
      <ChatInput
        status="sending"
        onSend={(): void => {
          return;
        }}
      />
    ),
  },
  ...chatInputStressStories,
];
