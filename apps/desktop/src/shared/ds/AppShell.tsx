import { useState, type ReactElement } from "react";
import { text } from "../lib";
import { shellDismiss } from "../platform/input";
import { Button } from "./Button";
import { Divider } from "./Divider";
import { IconButton } from "./IconButton";
import { ScrollArea } from "./ScrollArea";
import { Text } from "./Text";

export type PanelState = "open" | "closed";

type Props = {
  readonly list?: ReactElement | null;
  readonly rail?: ReactElement | null;
  readonly chat?: ReactElement | null;
  readonly status?: ReactElement | null;
  readonly children: ReactElement;
  readonly chatOpen?: PanelState;
  readonly defaultChatOpen?: PanelState;
  readonly onChatOpenChange?: (next: PanelState) => void;
  readonly listOpen?: PanelState;
  readonly defaultListOpen?: PanelState;
  readonly onListOpenChange?: (next: PanelState) => void;
};

const listPanelClasses = {
  closed:
    "hidden window-md:flex flex-none w-sidebar-width flex-col bg-surface scroll-flex",
  open: "flex flex-col bg-surface scroll-flex absolute top-space-1 bottom-space-1 left-sidebar-collapsed z-panel shadow-popover rounded-md w-sidebar-width max-w-full window-md:static window-md:z-auto window-md:shadow-none",
} as const;

const chatPanelClasses = {
  closed:
    "hidden window-lg:flex flex-none w-chat-width min-w-chat-width-min max-w-chat-width-max flex-col bg-surface scroll-flex",
  open: "flex flex-col bg-surface scroll-flex absolute top-space-1 bottom-space-1 right-space-2 z-panel shadow-popover rounded-md w-chat-width max-w-full window-lg:static window-lg:z-auto window-lg:shadow-none window-lg:min-w-chat-width-min window-lg:max-w-chat-width-max",
} as const;

const usePanelState = (
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

const ShellRail = (props: {
  readonly rail: ReactElement | null;
  readonly state: PanelState;
  readonly onChange: (next: PanelState) => void;
}): ReactElement => {
  const isOpen = props.state === "open";
  return (
    <aside
      aria-label={text("shell.stockList", "ko")}
      className="flex flex-col items-center gap-space-2 py-space-2 px-space-1 flex-none w-sidebar-collapsed bg-surface scroll-flex window-md:hidden"
    >
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
    </aside>
  );
};

const ShellSidebar = (props: {
  readonly list: ReactElement | null;
  readonly rail: ReactElement | null;
  readonly state: PanelState;
  readonly onChange: (next: PanelState) => void;
}): ReactElement | null => {
  if (props.list === null) return null;
  return (
    <>
      <ShellRail
        rail={props.rail}
        state={props.state}
        onChange={props.onChange}
      />
      <aside
        aria-label={text("shell.stockList", "ko")}
        className={listPanelClasses[props.state]}
      >
        {props.state === "open" ? (
          <div className="flex items-center justify-between gap-space-2 px-space-3 py-space-2 border-b-hairline border-solid border-line flex-none window-md:hidden">
            <Text variant="label">{text("shell.stockList", "ko")}</Text>
            <IconButton
              icon="panel-left-close"
              label={text("shell.closeList", "ko")}
              size="control-height-sm"
              onClick={(): void => {
                props.onChange("closed");
              }}
            />
          </div>
        ) : null}
        <ScrollArea direction="vertical">
          <div className="p-space-3 flex flex-col gap-space-2">
            {props.list}
          </div>
        </ScrollArea>
      </aside>
      <Divider direction="vertical" />
    </>
  );
};

const ShellChat = (props: {
  readonly chat: ReactElement | null;
  readonly state: PanelState;
  readonly onChange: (next: PanelState) => void;
}): ReactElement | null => {
  if (props.chat === null) return null;
  return (
    <>
      <div className="hidden window-lg:flex flex-none self-stretch">
        <Divider direction="vertical" />
      </div>
      <aside
        aria-label={text("shell.chat", "ko")}
        className={chatPanelClasses[props.state]}
      >
        {props.state === "open" ? (
          <div className="flex items-center justify-between gap-space-2 px-space-4 py-space-2 border-b-hairline border-solid border-line flex-none window-lg:hidden">
            <Text variant="label">{text("shell.chat", "ko")}</Text>
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
        <ScrollArea direction="vertical">
          <div className="p-space-4 flex flex-col gap-space-3">
            {props.chat}
          </div>
        </ScrollArea>
      </aside>
    </>
  );
};

const ShellChatBar = (props: {
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

export const AppShell = (props: Props): ReactElement => {
  const [chatState, updateChat] = usePanelState(
    props.chatOpen,
    props.defaultChatOpen,
    props.onChatOpenChange,
  );
  const [listState, updateList] = usePanelState(
    props.listOpen,
    props.defaultListOpen,
    props.onListOpenChange,
  );
  const listSlot = props.list ?? null;
  const railSlot = props.rail ?? null;
  const chatSlot = props.chat ?? null;
  const statusSlot = props.status ?? null;

  return (
    <div
      onKeyDown={(event): void => {
        if (shellDismiss(event) !== "dismiss") return;
        if (chatState === "open") updateChat("closed");
        if (listState === "open") updateList("closed");
      }}
      className="relative flex flex-col flex-1 w-full scroll-flex"
    >
      {statusSlot === null ? null : (
        <div className="flex-none w-full">{statusSlot}</div>
      )}
      <div className="relative flex flex-1 w-full scroll-flex">
        <ShellSidebar
          list={listSlot}
          rail={railSlot}
          state={listState}
          onChange={updateList}
        />
        <main className="relative flex-1 flex flex-col scroll-flex">
          <ShellChatBar
            chat={chatSlot}
            state={chatState}
            onChange={updateChat}
          />
          <ScrollArea direction="vertical">{props.children}</ScrollArea>
        </main>
        <ShellChat chat={chatSlot} state={chatState} onChange={updateChat} />
      </div>
    </div>
  );
};
