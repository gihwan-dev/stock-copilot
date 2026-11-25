import type { ReactElement } from "react";
import type { UiText } from "../lib";
import { Button } from "./Button";
import type { Action } from "./control-types";
import { Heading } from "./Heading";
import { ScrollArea } from "./ScrollArea";

type StepMark = "passed" | "now" | "next";
type Steps =
  readonly [StepMark, StepMark] | readonly [StepMark, StepMark, StepMark];

type Props = {
  readonly progress: Steps;
  readonly progressLabel: UiText;
  readonly title: UiText;
  readonly description: UiText;
  readonly children?: ReactElement | readonly ReactElement[];
  readonly primary?: Action;
  readonly secondary?: Action;
};

const Mark = (props: { readonly state: StepMark }): ReactElement => {
  switch (props.state) {
    case "passed":
      return <li className="p-space-1 rounded-pill bg-primary" />;
    case "now":
      return (
        <li
          aria-current="step"
          className="py-space-1 px-space-4 rounded-pill bg-primary"
        />
      );
    case "next":
      return <li className="p-space-1 rounded-pill bg-line-strong" />;
  }
};

const Progress = (props: {
  readonly progress: Steps;
  readonly label: UiText;
}): ReactElement => (
  <ol aria-label={props.label} className="flex items-center gap-space-2">
    <Mark state={props.progress[0]} />
    <Mark state={props.progress[1]} />
    {props.progress.length === 3 ? <Mark state={props.progress[2]} /> : null}
  </ol>
);

const Actions = (props: {
  readonly primary: Action | undefined;
  readonly secondary: Action | undefined;
}): ReactElement | null =>
  props.primary === undefined && props.secondary === undefined ? null : (
    <div className="flex flex-col gap-space-2">
      {props.primary === undefined ? null : (
        <Button
          variant="primary"
          onClick={props.primary.onPress}
          disabled={props.primary.disabled ?? "enabled"}
        >
          {props.primary.label}
        </Button>
      )}
      {props.secondary === undefined ? null : (
        <Button
          variant="ghost"
          onClick={props.secondary.onPress}
          disabled={props.secondary.disabled ?? "enabled"}
        >
          {props.secondary.label}
        </Button>
      )}
    </div>
  );

export const StepScreen = (props: Props): ReactElement => (
  <div className="flex flex-col flex-1 w-full max-w-full scroll-flex">
    <ScrollArea direction="vertical">
      <div className="flex flex-col items-center w-full px-space-6 pt-space-10 pb-space-6">
        <div className="flex flex-col gap-space-8 w-full max-w-dialog-width">
          <Progress progress={props.progress} label={props.progressLabel} />
          <div className="flex flex-col gap-space-2">
            <Heading level={1}>{props.title}</Heading>
            <p className="status-sentence text-body text-ink-muted">
              {props.description}
            </p>
          </div>
          <div className="flex flex-col gap-space-4">{props.children}</div>
        </div>
      </div>
    </ScrollArea>
    <div
      data-pinned-actions=""
      className="flex justify-center w-full px-space-6 pt-space-4 pb-space-8"
    >
      <div className="flex flex-col w-full max-w-dialog-width">
        <Actions primary={props.primary} secondary={props.secondary} />
      </div>
    </div>
  </div>
);
