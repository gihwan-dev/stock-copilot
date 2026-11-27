import { useState, type ReactElement } from "react";
import { text, type UiText } from "../lib";
import { Button } from "./Button";
import { IconButton } from "./IconButton";
import { ScrollArea } from "./ScrollArea";
import { Text } from "./Text";

export type PanelState = "open" | "closed";

export type AppShellProps = {
  readonly navigation?: ReactElement;
  readonly list?: ReactElement | null;
  readonly rail?: ReactElement | null;
  readonly chat?: ReactElement | null;
  readonly chatComposer?: ReactElement | null;
  readonly chatNotice?: ChatNoticeText | null;
  readonly status?: ReactElement | null;
  readonly children: ReactElement;
  readonly main?: "scroll" | "fill";
  readonly chatOpen?: PanelState;
  readonly defaultChatOpen?: PanelState;
  readonly onChatOpenChange?: (next: PanelState) => void;
  readonly listOpen?: PanelState;
  readonly defaultListOpen?: PanelState;
  readonly onListOpenChange?: (next: PanelState) => void;
};

export const ShellNavigation = (props: {
  readonly navigation: ReactElement | undefined;
}): ReactElement | null =>
  props.navigation === undefined ? null : (
    <nav
      aria-label={text("nav.destinations", "ko")}
      className="flex flex-none flex-col items-center gap-space-2 bg-surface p-space-1"
    >
      {props.navigation}
    </nav>
  );

export type ChatProps = {
  readonly chat: ReactElement;
  readonly composer: ReactElement | null | undefined;
  readonly notice: ChatNoticeText | null | undefined;
  readonly state: PanelState;
  readonly onChange: (next: PanelState) => void;
};

export const usePanelState = (
  controlled: PanelState | undefined,
  initial: PanelState | undefined,
  onChange: ((next: PanelState) => void) | undefined,
): readonly [PanelState, (next: PanelState) => void] => {
  const [uncontrolled, setUncontrolled] = useState<PanelState>(
    initial ?? "closed",
  );
  const state = controlled ?? uncontrolled;
  const update = (next: PanelState): void => {
    if (controlled === undefined) setUncontrolled(next);
    if (onChange !== undefined) onChange(next);
  };
  return [state, update];
};

export const ListHead = (props: {
  readonly onClose: () => void;
}): ReactElement => (
  <div className="flex items-center justify-between gap-space-2 px-space-3 py-space-2 border-b-hairline border-solid border-line flex-none window-md:hidden">
    <Text variant="label">{text("shell.stockList", "ko")}</Text>
    <IconButton
      icon="panel-left-close"
      label={text("shell.closeList", "ko")}
      size="control-height-sm"
      onClick={props.onClose}
    />
  </div>
);

export const ChatFooter = (props: {
  readonly composer: ReactElement | null;
}): ReactElement | null =>
  props.composer === null ? null : (
    <div className="flex-none px-space-3 pt-space-2 pb-space-3 border-t-hairline border-solid border-line bg-surface">
      {props.composer}
    </div>
  );

export type ChatNoticeText = {
  readonly summary: UiText;
  readonly detail: UiText;
};

const ChatNotice = (props: ChatNoticeText): ReactElement => {
  const [state, setState] = useState<PanelState>("closed");
  const isOpen = state === "open";
  return (
    <div className="flex flex-col gap-space-1">
      <div className="flex items-center gap-space-1">
        <p className="status-sentence flex-1 text-caption text-ink-subtle">
          {props.summary}
        </p>
        <IconButton
          icon={isOpen ? "chevron-up" : "chevron-down"}
          label={
            isOpen
              ? text("shell.hideChatNotice", "ko")
              : text("shell.showChatNotice", "ko")
          }
          expanded={state}
          size="control-height-sm"
          onClick={(): void => {
            setState(isOpen ? "closed" : "open");
          }}
        />
      </div>
      <p
        hidden={!isOpen}
        className="status-sentence text-caption text-ink-subtle"
      >
        {props.detail}
      </p>
    </div>
  );
};

export const ChatHead = (props: {
  readonly notice: ChatNoticeText | null;
  readonly state: PanelState;
  readonly onChange: (next: PanelState) => void;
}): ReactElement | null => {
  if (props.notice === null && props.state !== "open") return null;
  return (
    <div className="flex flex-col gap-space-1 px-space-4 pt-space-3 pb-space-2 border-b-hairline border-solid border-line flex-none">
      <div className="flex items-center justify-between gap-space-2">
        <Text variant="label">{text("shell.chat", "ko")}</Text>
        {props.state === "open" ? (
          <div className="flex window-lg:hidden">
            <IconButton
              icon="x"
              label={text("shell.closeChat", "ko")}
              size="control-height-sm"
              onClick={(): void => {
                props.onChange("closed");
              }}
            />
          </div>
        ) : null}
      </div>
      {props.notice === null ? null : (
        <ChatNotice
          summary={props.notice.summary}
          detail={props.notice.detail}
        />
      )}
    </div>
  );
};

export const ShellChatBar = (props: {
  readonly chat: ReactElement | null;
  readonly state: PanelState;
  readonly onChange: (next: PanelState) => void;
}): ReactElement | null => {
  if (props.chat === null) return null;
  const isOpen = props.state === "open";
  return (
    <div className="flex items-center justify-end gap-space-2 px-space-3 py-space-1 bg-surface border-b-hairline border-solid border-line flex-none window-lg:hidden">
      <Button
        variant="ghost"
        size="control-height-sm"
        onClick={(): void => {
          props.onChange(isOpen ? "closed" : "open");
        }}
      >
        {isOpen ? text("shell.closeChat", "ko") : text("shell.openChat", "ko")}
      </Button>
    </div>
  );
};

export const RailContent = (props: {
  readonly rail: ReactElement | null;
  readonly state: PanelState;
  readonly onChange: (next: PanelState) => void;
}): ReactElement => {
  const isOpen = props.state === "open";
  return (
    <>
      <IconButton
        icon={isOpen ? "panel-left-close" : "panel-left-open"}
        label={
          isOpen ? text("shell.closeList", "ko") : text("shell.openList", "ko")
        }
        expanded={props.state}
        onClick={(): void => {
          props.onChange(isOpen ? "closed" : "open");
        }}
        size="control-height-sm"
        tooltipSide="right"
      />
      {props.rail === null ? null : (
        <ScrollArea direction="vertical">
          <div className="flex flex-col items-center gap-space-2">
            {props.rail}
          </div>
        </ScrollArea>
      )}
    </>
  );
};
