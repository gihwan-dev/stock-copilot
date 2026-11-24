import type { Action } from "./control-types";
import { Button } from "./Button";
import type { ReactElement } from "react";
import type { UiText } from "../lib";
import { Heading } from "./Heading";
import { ScrollArea as CardBody } from "./ScrollArea";
type Props = {
  readonly title?: UiText;
  readonly actions?: Action;
  readonly children: ReactElement | readonly ReactElement[];
};
export const Card = (props: Props): ReactElement => (
  <section className="bg-surface border-hairline border-solid border-line rounded-lg flex flex-col max-w-full scroll-flex">
    {props.title !== undefined || props.actions !== undefined ? (
      <header className="flex flex-wrap items-center gap-space-2 px-space-4 pt-space-3 min-h-toolbar-height">
        {props.title === undefined ? null : (
          <div className="flex-1 scroll-flex">
            <Heading level={3}>{props.title}</Heading>
          </div>
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
    ) : null}
    <CardBody direction="horizontal">
      <div className="p-space-4 flex flex-col gap-space-4">
        {props.children}
      </div>
    </CardBody>
  </section>
);
