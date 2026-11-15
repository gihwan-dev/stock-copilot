import { useState, type ReactElement } from "react";
import { text } from "../lib";
import { Card } from "./Card";
import { Table } from "./Table";
import { Dialog } from "./Dialog";
import { EmptyState } from "./EmptyState";
import { Skeleton } from "./Skeleton";
import { Text } from "./Text";
import type { Action } from "./control-types";
import { TextField } from "./TextField";
type Story = {
  readonly component: "Card" | "Table" | "Dialog" | "EmptyState" | "Skeleton";
  readonly state: "normal" | "empty" | "open" | "closed";
  readonly themes: readonly ["light", "dark"];
  readonly render: () => ReactElement;
};
const title = text("app.title", "ko");
const description = text("ds.description", "ko");
const action: Action = {
  label: text("ds.close", "ko"),
  onPress: (): void => {
    return;
  },
};
const content = <Text>{description}</Text>;
const DialogStory = (props: {
  readonly initial: "open" | "closed";
  readonly wide: "normal" | "wide";
}): ReactElement => {
  const [open, setOpen] = useState(props.initial);
  return (
    <Dialog
      open={open}
      wide={props.wide}
      title={title}
      onClose={(): void => {
        setOpen("closed");
      }}
      actions={{
        cancel: {
          label: action.label,
          onPress: (): void => {
            setOpen("closed");
          },
        },
      }}
    >
      <TextField label={description} />
    </Dialog>
  );
};
export const feedbackComponentStories: readonly Story[] = [
  ...[
    (): ReactElement => <Card>{content}</Card>,
    (): ReactElement => <Card title={title}>{content}</Card>,
    (): ReactElement => <Card actions={action}>{content}</Card>,
    (): ReactElement => (
      <Card title={title} actions={action}>
        {content}
      </Card>
    ),
  ].map((render): Story => ({
    component: "Card",
    state: "normal",
    themes: ["light", "dark"],
    render,
  })),
  ...(["normal", "dense"] as const).map((dense): Story => ({
    component: "Table",
    state: "normal",
    themes: ["light", "dark"],
    render: (): ReactElement => (
      <Table.Root caption={title} dense={dense}>
        <Table.Head>
          <Table.Row>
            <Table.Header>{title}</Table.Header>
            <Table.Header align="end">{description}</Table.Header>
          </Table.Row>
        </Table.Head>
        <Table.Body>
          <Table.Row>
            <Table.RowHeader>{title}</Table.RowHeader>
            <Table.Cell align="end">{content}</Table.Cell>
          </Table.Row>
        </Table.Body>
      </Table.Root>
    ),
  })),
  {
    component: "Table",
    state: "empty",
    themes: ["light", "dark"],
    render: (): ReactElement => <Table.Root caption={title} />,
  },
  ...(["normal", "wide"] as const).flatMap((wide) =>
    (["open", "closed"] as const).map((initial): Story => ({
      component: "Dialog",
      state: initial,
      themes: ["light", "dark"],
      render: (): ReactElement => <DialogStory initial={initial} wide={wide} />,
    })),
  ),
  ...[
    (): ReactElement => <EmptyState title={title} />,
    (): ReactElement => <EmptyState title={title}>{description}</EmptyState>,
    (): ReactElement => <EmptyState title={title} icon="wallet" />,
    (): ReactElement => (
      <EmptyState title={title} icon="wallet" action={action}>
        {description}
      </EmptyState>
    ),
  ].map((render): Story => ({
    component: "EmptyState",
    state: "empty",
    themes: ["light", "dark"],
    render,
  })),
  ...[
    (): ReactElement => <Skeleton />,
    (): ReactElement => <Skeleton lines={6} />,
    (): ReactElement => <Skeleton variant="block" height="chart-height-sm" />,
    (): ReactElement => <Skeleton variant="block" height="chart-height" />,
    (): ReactElement => <Skeleton variant="block" height="chart-height-lg" />,
  ].map((render): Story => ({
    component: "Skeleton",
    state: "normal",
    themes: ["light", "dark"],
    render,
  })),
];
