import type { ReactElement } from "react";
import type { UiText } from "../lib";
import { Button } from "./Button";
import type { Action } from "./control-types";
import { Heading } from "./Heading";
import type { LayoutChildren } from "./layout-children";

type Layout = "scroll" | "fill";

type Props = LayoutChildren & {
  readonly title: UiText;
  readonly actions?: Action;
  readonly layout?: Layout;
};

const rootClasses = {
  scroll: "@container flex flex-col w-full max-w-full scroll-flex",
  fill: "@container flex flex-col flex-1 w-full max-w-full scroll-flex",
} as const;

const bodyClasses = {
  scroll:
    "flex flex-col items-center w-full max-w-full scroll-flex p-space-3 @content-medium:p-space-6",
  fill: "flex flex-col items-center flex-1 w-full max-w-full scroll-flex p-space-3 @content-medium:p-space-6",
} as const;

const innerClasses = {
  scroll: "flex flex-col gap-space-6 w-full max-w-content-max scroll-flex",
  fill: "flex flex-col flex-1 gap-space-4 w-full max-w-content-max scroll-flex",
} as const;

export const Page = (props: Props): ReactElement => {
  const layout = props.layout ?? "scroll";
  return (
    <div className={rootClasses[layout]}>
      <header className="flex flex-wrap items-center gap-space-2 px-space-3 @content-medium:px-space-6 py-space-1 min-h-toolbar-height border-b-hairline border-solid border-line bg-surface w-full max-w-full scroll-flex">
        <div className="flex-1 wrap-slot scroll-flex py-gap-hairline">
          <Heading level={1}>{props.title}</Heading>
        </div>
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
      <div className={bodyClasses[layout]}>
        <div className={innerClasses[layout]}>{props.children}</div>
      </div>
    </div>
  );
};
