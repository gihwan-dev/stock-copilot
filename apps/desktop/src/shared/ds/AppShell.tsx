import type { ReactElement } from "react";
import { text } from "../lib";
import { shellDismiss } from "../platform/input";
import { Divider } from "./Divider";
import { ScrollArea } from "./ScrollArea";
import {
  ChatFooter,
  ChatHead,
  ListHead,
  RailContent,
  ShellChatBar,
  usePanelState,
  type AppShellProps,
  type ChatProps,
  type PanelState,
} from "./ShellParts";
export type { PanelState } from "./ShellParts";
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
const ShellRail = (props: {
  readonly rail: ReactElement | null;
  readonly state: PanelState;
  readonly onChange: (next: PanelState) => void;
}): ReactElement => (
  <aside
    aria-label={text("shell.stockList", "ko")}
    className="flex flex-col items-center gap-space-2 py-space-2 px-space-1 flex-none w-sidebar-collapsed bg-surface scroll-flex window-md:hidden"
  >
    <RailContent
      rail={props.rail}
      state={props.state}
      onChange={props.onChange}
    />
  </aside>
);
const ShellSidebar = (props: {
  readonly list: ReactElement | null | undefined;
  readonly rail: ReactElement | null | undefined;
  readonly state: PanelState;
  readonly onChange: (next: PanelState) => void;
}): ReactElement | null => {
  if (props.list === undefined || props.list === null) return null;
  return (
    <>
      <ShellRail
        rail={props.rail ?? null}
        state={props.state}
        onChange={props.onChange}
      />
      <aside
        aria-label={text("shell.stockList", "ko")}
        className={listPanelClasses[props.state]}
      >
        {props.state === "open" ? (
          <ListHead
            onClose={(): void => {
              props.onChange("closed");
            }}
          />
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
const ChatBody = (props: {
  readonly chat: ReactElement;
  readonly composer: ReactElement | null;
}): ReactElement => {
  return (
    <>
      <div className="flex flex-col flex-1 scroll-flex">
        <ScrollArea direction="vertical">
          <div className="p-space-4 flex flex-col gap-space-3">
            {props.chat}
          </div>
        </ScrollArea>
      </div>
      <ChatFooter composer={props.composer} />
    </>
  );
};
const ShellChat = (
  props: ChatProps & { readonly main: ReactElement },
): ReactElement => {
  const inner = (
    <>
      <ChatHead
        notice={props.notice ?? null}
        state={props.state}
        onChange={props.onChange}
      />
      <ChatBody chat={props.chat} composer={props.composer ?? null} />
    </>
  );
  return (
    <>
      {props.main}
      <div className="hidden window-lg:flex flex-none self-stretch">
        <Divider direction="vertical" />
      </div>
      <aside
        aria-label={text("shell.chat", "ko")}
        className={chatPanelClasses[props.state]}
      >
        {inner}
      </aside>
    </>
  );
};
export const AppShell = (props: AppShellProps): ReactElement => {
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
  const chatSlot = props.chat ?? null;
  const statusSlot = props.status ?? null;
  const main = (
    <main className="relative flex-1 flex flex-col scroll-flex">
      <ShellChatBar chat={chatSlot} state={chatState} onChange={updateChat} />
      {props.main === "fill" ? (
        <div className="flex-1 flex flex-col scroll-flex">{props.children}</div>
      ) : (
        <ScrollArea direction="vertical">{props.children}</ScrollArea>
      )}
    </main>
  );
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
          list={props.list}
          rail={props.rail}
          state={listState}
          onChange={updateList}
        />
        {chatSlot === null ? (
          main
        ) : (
          <ShellChat
            main={main}
            chat={chatSlot}
            composer={props.chatComposer}
            notice={props.chatNotice}
            state={chatState}
            onChange={updateChat}
          />
        )}
      </div>
    </div>
  );
};
