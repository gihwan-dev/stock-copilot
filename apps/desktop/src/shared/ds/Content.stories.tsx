import type { ReactElement } from "react";
import { text, parseFreeText, emptyFreeText, type FreeText } from "../lib";
import { Markdown } from "./Markdown";
import { ChatInput } from "./ChatInput";
const example = parseFreeText(text("chat.example", "ko"));
const content = example.kind === "ok" ? example.value : emptyFreeText;
type Story = {
  readonly component: "Markdown" | "ChatInput";
  readonly state: "normal" | "denied" | "sending" | "stress";
  readonly themes: readonly ["light", "dark"];
  readonly draft?: FreeText;
  readonly render: () => ReactElement;
};
const longDraft = parseFreeText(text("chat.label", "ko").repeat(1000));
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
          linkAllowed={(): "deny" => "deny"}
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
  {
    component: "ChatInput",
    state: "stress",
    themes: ["light", "dark"],
    draft: longDraft.kind === "ok" ? longDraft.value : emptyFreeText,
    render: (): ReactElement => (
      <ChatInput
        onSend={(): void => {
          return;
        }}
      />
    ),
  },
];
