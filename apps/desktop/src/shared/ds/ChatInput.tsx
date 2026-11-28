import { useState, type ReactElement } from "react";
import { chatCommit, chatEventValue } from "../platform/input";
import { emptyFreeText, parseFreeText, text, type FreeText } from "../lib";
import { Button } from "./Button";
type Props = {
  readonly onSend: (value: FreeText) => void;
  readonly status?: "ready" | "sending";
  readonly draft?: FreeText;
  readonly onDraftChange?: (value: FreeText) => void;
  readonly secondaryAction?: ReactElement | undefined;
  readonly layout?: "stacked" | "compact";
};
type EditorProps = {
  readonly onSend: (value: FreeText) => void;
  readonly status?: "ready" | "sending";
  readonly initialDraft: FreeText;
  readonly draft?: FreeText | undefined;
  readonly onDraftChange?: ((value: FreeText) => void) | undefined;
  readonly secondaryAction?: ReactElement | undefined;
  readonly layout?: "stacked" | "compact" | undefined;
};
const useDraft = (
  props: EditorProps,
): readonly [FreeText, (next: FreeText) => void] => {
  const [inner, setInner] = useState(props.initialDraft);
  const update = (next: FreeText): void => {
    setInner(next);
    if (props.onDraftChange !== undefined) props.onDraftChange(next);
  };
  return [props.draft ?? inner, update];
};
type Composition = "idle" | "composing" | "ending";
const ChatField = (props: {
  readonly value: FreeText;
  readonly status: "ready" | "sending";
  readonly compact: "compact" | "stacked";
  readonly composition: Composition;
  readonly onComposition: (next: Composition) => void;
  readonly onValue: (next: FreeText) => void;
  readonly onCommit: () => void;
}): ReactElement => {
  const composition = props.composition;
  const setComposition = props.onComposition;
  const resetComposition = (): void => {
    setComposition("idle");
  };
  return (
    <textarea
      value={props.value}
      aria-label={
        props.compact === "compact" ? text("chat.label", "ko") : undefined
      }
      placeholder={
        props.compact === "compact" ? text("chat.placeholder", "ko") : undefined
      }
      readOnly={props.status === "sending"}
      onChange={(event): void => {
        const next = chatEventValue(event);
        if (props.status !== "sending" && next.kind === "ok")
          props.onValue(next.value);
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
        if (chatCommit(event, composition) === "commit") props.onCommit();
        if (composition === "ending") setComposition("idle");
      }}
      className="status-sentence w-full chat-content chat-input rounded-md border-hairline border-solid border-line-strong bg-surface-sunken px-space-3 py-space-2 text-body focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:outline-offset-focus disabled:opacity-disabled"
    />
  );
};
const CompactActions = (props: {
  readonly secondaryAction: ReactElement | undefined;
  readonly send: ReactElement;
}): ReactElement => (
  <div className="flex items-center gap-space-2">
    <p className="flex-1 status-sentence text-caption text-ink-subtle">
      {text("chat.hint", "ko")}
    </p>
    <div className="flex flex-1 justify-end">{props.secondaryAction}</div>
    <div className="flex flex-1">{props.send}</div>
  </div>
);
const ChatEditor = (props: EditorProps): ReactElement => {
  const [value, setValue] = useDraft(props);
  const status = props.status ?? "ready";
  const compact = props.layout === "compact" ? "compact" : "stacked";
  const [composition, setComposition] = useState<Composition>("idle");
  const send = (): void => {
    if (
      status === "sending" ||
      value.trim() === "" ||
      composition === "composing"
    )
      return;
    setComposition("idle");
    setValue(emptyFreeText);
    props.onSend(value);
  };
  const field = (
    <ChatField
      value={value}
      status={status}
      compact={compact}
      composition={composition}
      onComposition={setComposition}
      onValue={setValue}
      onCommit={send}
    />
  );
  const button = (
    <Button
      size={compact === "compact" ? "control-height-sm" : "control-height"}
      disabled={
        status === "sending" || value.trim() === "" ? "disabled" : "enabled"
      }
      onClick={send}
    >
      {text(status === "sending" ? "chat.sending" : "chat.send", "ko")}
    </Button>
  );
  return compact === "compact" ? (
    <div className="flex status-sentence flex-col gap-space-2 text-ink">
      {field}
      <CompactActions secondaryAction={props.secondaryAction} send={button} />
    </div>
  ) : (
    <div className="flex status-sentence flex-col gap-space-2 text-ink">
      <label className="flex status-sentence flex-col gap-space-1 text-label">
        {text("chat.label", "ko")}
        {field}
      </label>
      {button}
    </div>
  );
};

export const ChatInput = (props: Props): ReactElement => (
  <ChatEditor
    onSend={props.onSend}
    status={props.status ?? "ready"}
    initialDraft={emptyFreeText}
    draft={props.draft}
    onDraftChange={props.onDraftChange}
    secondaryAction={props.secondaryAction}
    layout={props.layout}
  />
);
const longDraft = parseFreeText((text("chat.label", "ko") + "\n").repeat(1000));
type DraftStory = {
  readonly component: "ChatInput";
  readonly state: "stress";
  readonly themes: readonly ["light", "dark"];
  readonly render: () => ReactElement;
};
export const chatInputStressWithDraft = (
  draft: FreeText,
): readonly DraftStory[] => [
  {
    component: "ChatInput",
    state: "stress",
    themes: ["light", "dark"],
    render: (): ReactElement => (
      <ChatEditor
        initialDraft={draft}
        onSend={(): void => {
          return;
        }}
      />
    ),
  },
];
export const chatInputStressStories: readonly DraftStory[] =
  chatInputStressWithDraft(
    longDraft.kind === "ok" ? longDraft.value : emptyFreeText,
  );
