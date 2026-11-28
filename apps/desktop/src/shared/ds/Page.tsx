import type { ReactElement } from "react";
import type { UiText } from "../lib";
import { Button } from "./Button";
import type { Action } from "./control-types";
import { ChatBody } from "./AppShell";
import { ScrollArea } from "./ScrollArea";
import { Heading } from "./Heading";
import type { LayoutChildren } from "./layout-children";

type Layout = "scroll" | "fill" | "reading";

type Props = LayoutChildren & {
  readonly title: UiText;
  readonly conversation?: {
    readonly messages: ReactElement;
    readonly composer: ReactElement | null;
  };
  readonly actions?: Action;
  readonly navigation?: ReactElement;
  readonly layout?: Layout;
};

const rootClasses = {
  reading: "@container flex flex-col flex-1 w-full max-w-full scroll-flex",
  scroll: "@container flex flex-col w-full max-w-full scroll-flex",
  fill: "@container flex flex-col flex-1 w-full max-w-full scroll-flex",
} as const;

const bodyClasses = {
  reading:
    "flex flex-col items-center w-full max-w-full scroll-flex p-space-3 @content-medium:p-space-6",
  scroll:
    "flex flex-col items-center w-full max-w-full scroll-flex p-space-3 @content-medium:p-space-6",
  fill: "flex flex-col items-center flex-1 w-full max-w-full scroll-flex p-space-3 @content-medium:p-space-6",
} as const;

const innerClasses = {
  reading: "flex flex-col gap-space-6 w-full max-w-content-max scroll-flex",
  scroll: "flex flex-col gap-space-6 w-full max-w-content-max scroll-flex",
  fill: "flex flex-col flex-1 gap-space-4 w-full max-w-content-max scroll-flex",
} as const;

export const Page = (props: Props): ReactElement => {
  const layout = props.layout ?? "scroll";
  const body = (
    <div className={bodyClasses[layout]}>
      <div className={innerClasses[layout]}>
        {props.children}
        {props.conversation === undefined ? null : (
          <ChatBody
            chat={props.conversation.messages}
            composer={props.conversation.composer}
          />
        )}
      </div>
    </div>
  );
  return (
    <div className={rootClasses[layout]}>
      <header className="flex flex-wrap items-center gap-space-2 px-space-3 @content-medium:px-space-6 py-space-1 min-h-toolbar-height border-b-hairline border-solid border-line bg-surface w-full max-w-full scroll-flex">
        <div className="flex-1 wrap-slot scroll-flex py-gap-hairline">
          <Heading level={1}>{props.title}</Heading>
        </div>
        {props.navigation === undefined ? null : (
          <div className="max-w-full scroll-flex">{props.navigation}</div>
        )}
        {props.actions === undefined ? null : (
          <Button
            size="control-height-sm"
            onClick={props.actions.onPress}
            disabled={props.actions.disabled ?? "enabled"}
          >
            {props.actions.label}
          </Button>
        )}
      </header>
      {layout !== "reading" ? (
        body
      ) : (
        <ScrollArea direction="vertical">{body}</ScrollArea>
      )}
    </div>
  );
};
