import { useState, type ReactElement } from "react";
import { chatCommit, chatEventValue } from "../platform/input";
import { emptyFreeText, text, type FreeText } from "../lib";
import { Button } from "./Button";
type Props = {
  readonly onSend: (value: FreeText) => void;
  readonly status?: "ready" | "sending";
};
export const ChatInput = (props: Props): ReactElement => {
  const [value, setValue] = useState(emptyFreeText);
  const [composition, setComposition] = useState<
    "idle" | "composing" | "ending"
  >("idle");
  const send = (): void => {
    if (
      props.status === "sending" ||
      value.trim() === "" ||
      composition === "composing"
    )
      return;
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
