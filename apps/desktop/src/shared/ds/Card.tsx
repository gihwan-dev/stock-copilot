import type { Action } from "./control-types";
import { Button } from "./Button";
import type { ReactElement } from "react";
import type { UiText } from "../lib";
import { Heading } from "./Heading";
import { ScrollArea as CardBody } from "./ScrollArea";
import { ScrollOwnerScope } from "./scroll-owner";
type Layout = "flow" | "fill";
type Props = {
  readonly title?: UiText;
  readonly actions?: Action;
  readonly meta?: ReactElement | undefined;
  readonly layout?: Layout;
  readonly children: ReactElement | readonly ReactElement[];
};
type HeaderState = "shown" | "hidden";
const Body = (props: {
  readonly layout: Layout;
  readonly header: HeaderState;
  readonly children: ReactElement | readonly ReactElement[];
}): ReactElement =>
  props.layout === "fill" ? (
    <ScrollOwnerScope owner="card">
      <CardBody direction="both">
        <div
          className={
            props.header === "shown"
              ? "px-space-4 pt-space-1 pb-space-4 flex flex-col gap-space-4"
              : "p-space-4 flex flex-col gap-space-4"
          }
        >
          {props.children}
        </div>
      </CardBody>
    </ScrollOwnerScope>
  ) : (
    <ScrollOwnerScope owner="own">
      <CardBody direction="horizontal">
        <div className="p-space-4 flex flex-col gap-space-4">
          {props.children}
        </div>
      </CardBody>
    </ScrollOwnerScope>
  );
const CardHeader = (props: {
  readonly meta?: ReactElement | undefined;
  readonly title: UiText | undefined;
  readonly actions: Action | undefined;
}): ReactElement => (
  <header className="flex flex-wrap items-center gap-space-2 px-space-4 pt-space-3 min-h-toolbar-height">
    {props.title === undefined ? null : (
      <div className="flex-1 scroll-flex">
        <Heading level={3}>{props.title}</Heading>
      </div>
    )}
    {props.meta === undefined ? null : (
      <div className="max-w-full scroll-flex">{props.meta}</div>
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
);
export const Card = (props: Props): ReactElement => {
  const layout = props.layout ?? "flow";
  const header: HeaderState =
    props.title !== undefined || props.actions !== undefined
      ? "shown"
      : "hidden";
  return (
    <section
      className={
        layout === "fill"
          ? "bg-surface border-hairline border-solid border-line rounded-lg flex flex-col h-full max-w-full scroll-flex"
          : "bg-surface border-hairline border-solid border-line rounded-lg flex flex-col max-w-full scroll-flex"
      }
    >
      {header === "shown" ? (
        <CardHeader
          meta={props.meta}
          title={props.title}
          actions={props.actions}
        />
      ) : null}
      <Body layout={layout} header={header}>
        {props.children}
      </Body>
    </section>
  );
};
