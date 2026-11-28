import { SectionTarget, type SectionPosition } from "./SectionTarget";
import type { Action } from "./control-types";
import { Button } from "./Button";
import type { ReactElement } from "react";
import { useWidgetAppearance } from "./AppFrame";
import { useWidgetTitle } from "./WidgetFrame";
import type { UiText } from "../lib";
import { Heading } from "./Heading";
import { ScrollArea as CardBody } from "./ScrollArea";
import { ScrollOwnerScope } from "./scroll-owner";
type Inset = "0" | "4" | "8" | "16";
type Corner = "0" | "4" | "8" | "12";
type Layout = "flow" | "fill";
type Props = {
  readonly position?: SectionPosition | undefined;
  readonly title?: UiText | undefined;
  readonly actions?: Action | undefined;
  readonly meta?: ReactElement | undefined;
  readonly layout?: Layout;
  readonly children: ReactElement | readonly ReactElement[];
};
const bodyClasses = {
  "0": "flex flex-col gap-space-2",
  "4": "flex flex-col gap-space-2 p-space-1",
  "8": "flex flex-col gap-space-2 p-space-2",
  "16": "flex flex-col gap-space-2 p-space-4",
} as const;
const headerClasses = {
  "0": "flex flex-none flex-wrap items-center gap-space-2",
  "4": "flex flex-none flex-wrap items-center gap-space-2 px-space-1 pt-space-1",
  "8": "flex flex-none flex-wrap items-center gap-space-2 px-space-2 pt-space-2",
  "16": "flex flex-none flex-wrap items-center gap-space-2 px-space-4 pt-space-4",
} as const;
const flowClasses = {
  "4": "flex flex-col bg-surface border-solid border-line border-hairline max-w-full scroll-flex rounded-sm",
  "8": "flex flex-col bg-surface border-solid border-line border-hairline max-w-full scroll-flex rounded-md",
  "12": "flex flex-col bg-surface border-solid border-line border-hairline max-w-full scroll-flex rounded-lg",
} as const;
const fillClasses = {
  "4": "flex flex-col bg-surface border-solid border-line border-hairline h-full max-w-full scroll-flex rounded-sm",
  "8": "flex flex-col bg-surface border-solid border-line border-hairline h-full max-w-full scroll-flex rounded-md",
  "12": "flex flex-col bg-surface border-solid border-line border-hairline h-full max-w-full scroll-flex rounded-lg",
} as const;
const Body = (props: {
  readonly inset: Inset;
  readonly layout: Layout;
  readonly children: ReactElement | readonly ReactElement[];
}): ReactElement => {
  return props.layout === "fill" ? (
    <ScrollOwnerScope owner="card">
      <CardBody direction="both">
        <div data-ui-card-body="" className={bodyClasses[props.inset]}>
          {props.children}
        </div>
      </CardBody>
    </ScrollOwnerScope>
  ) : (
    <ScrollOwnerScope owner="own">
      <CardBody direction="horizontal">
        <div data-ui-card-body="" className={bodyClasses[props.inset]}>
          {props.children}
        </div>
      </CardBody>
    </ScrollOwnerScope>
  );
};
const CardHeader = (props: {
  readonly inset: Inset;
  readonly meta?: ReactElement | undefined;
  readonly title: UiText | undefined;
  readonly actions: Action | undefined;
}): ReactElement => {
  return (
    <header data-ui-card-header="" className={headerClasses[props.inset]}>
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
};
const WidgetCard = (props: Props): ReactElement => (
  <SectionTarget position={props.position}>
    <section
      data-widget-card=""
      className="flex flex-col flex-1 w-full max-w-full gap-space-1"
    >
      {props.title === undefined &&
      props.meta === undefined &&
      props.actions === undefined ? null : (
        <CardHeader
          inset="0"
          title={props.title}
          meta={props.meta}
          actions={props.actions}
        />
      )}
      <ScrollOwnerScope owner="card">
        <div
          data-widget-body=""
          className="flex flex-1 flex-col gap-space-2 w-full max-w-full"
        >
          {props.children}
        </div>
      </ScrollOwnerScope>
    </section>
  </SectionTarget>
);

const RoundedSurface = (props: {
  readonly corner: "4" | "8" | "12";
  readonly layout: Layout;
  readonly children: ReactElement;
}): ReactElement => (
  <section
    data-ui-card=""
    className={
      props.layout === "fill"
        ? fillClasses[props.corner]
        : flowClasses[props.corner]
    }
  >
    {props.children}
  </section>
);

const CardSurface = (props: {
  readonly corner: Corner;
  readonly layout: Layout;
  readonly children: ReactElement;
}): ReactElement => {
  if (props.corner === "0")
    return (
      <section
        data-ui-card=""
        className={
          props.layout === "fill"
            ? "bg-surface border-hairline border-line border-solid flex flex-col h-full max-w-full scroll-flex"
            : "bg-surface border-hairline border-line border-solid flex flex-col max-w-full scroll-flex"
        }
      >
        {props.children}
      </section>
    );
  return (
    <RoundedSurface corner={props.corner} layout={props.layout}>
      {props.children}
    </RoundedSurface>
  );
};

export const Card = (props: Props): ReactElement => {
  const widget = useWidgetTitle();
  const appearance = useWidgetAppearance();
  const layout = props.layout ?? "flow";
  if (widget !== undefined) {
    return (
      <WidgetCard
        title={props.title === widget ? undefined : props.title}
        actions={props.actions}
        meta={props.meta}
        position={props.position}
      >
        {props.children}
      </WidgetCard>
    );
  }
  return (
    <SectionTarget position={props.position}>
      <CardSurface corner={appearance.radius} layout={layout}>
        <>
          {props.title !== undefined || props.actions !== undefined ? (
            <CardHeader
              inset={appearance.padding}
              meta={props.meta}
              title={props.title}
              actions={props.actions}
            />
          ) : null}
          <Body inset={appearance.padding} layout={layout}>
            {props.children}
          </Body>
        </>
      </CardSurface>
    </SectionTarget>
  );
};
