import { useState, type ReactElement } from "react";
import { chatCommit, chatEventValue } from "../platform/input";
import { emptyFreeText, parseFreeText, text, type FreeText } from "../lib";
import { Button } from "./Button";
type Props = {
  readonly onSend: (value: FreeText) => void;
  readonly status?: "ready" | "sending";
};
type EditorProps = Props & { readonly initialDraft: FreeText };
const ChatEditor = (props: EditorProps): ReactElement => {
  const [value, setValue] = useState(props.initialDraft);
  const [composition, setComposition] = useState<
    "idle" | "composing" | "ending"
  >("idle");
  const resetComposition = (): void => {
    setComposition("idle");
  };
  const send = (): void => {
    if (
      props.status === "sending" ||
      value.trim() === "" ||
      composition === "composing"
    )
      return;
    setComposition("idle");
    setValue(emptyFreeText);
    props.onSend(value);
  };
  return (
    <div className="flex status-sentence flex-col gap-space-2 text-ink">
      <label className="flex status-sentence flex-col gap-space-1 text-label">
        {text("chat.label", "ko")}
        <textarea
          value={value}
          readOnly={props.status === "sending"}
          onChange={(event): void => {
            const next = chatEventValue(event);
            if (props.status !== "sending" && next.kind === "ok") {
              setValue(next.value);
            }
          }}
          onCompositionStart={(): void => {
            setComposition("composing");
          }}
          onCompositionEnd={(): void => {
            setComposition("ending");
          }}
          onBlur={resetComposition}
          onFocus={resetComposition}
          onKeyDown={(event): void => {
            if (chatCommit(event, composition) === "commit") send();
            if (composition === "ending") setComposition("idle");
          }}
          className="status-sentence w-full chat-content chat-input rounded-md border-hairline border-solid border-line-strong bg-surface-sunken px-space-3 py-space-2 text-body focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:outline-offset-focus disabled:opacity-disabled"
        />
      </label>
      <Button
        disabled={
          props.status === "sending" || value.trim() === ""
            ? "disabled"
            : "enabled"
        }
        onClick={send}
      >
        {text(props.status === "sending" ? "chat.sending" : "chat.send", "ko")}
      </Button>
    </div>
  );
};

export const ChatInput = (props: Props): ReactElement => (
  <ChatEditor
    onSend={props.onSend}
    status={props.status ?? "ready"}
    initialDraft={emptyFreeText}
  />
);
const longDraft = parseFreeText((text("chat.label", "ko") + "\n").repeat(1000));
type DraftStory = {
  readonly component: "ChatInput";
  readonly state: "stress";
  readonly themes: readonly ["light", "dark"];
  readonly render: () => ReactElement;
};
export const chatInputStressStories: readonly DraftStory[] = [
  {
    component: "ChatInput",
    state: "stress",
    themes: ["light", "dark"],
    render: (): ReactElement => (
      <ChatEditor
        initialDraft={longDraft.kind === "ok" ? longDraft.value : emptyFreeText}
        onSend={(): void => {
          return;
        }}
      />
    ),
  },
];
