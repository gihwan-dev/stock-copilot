import { useId, useState, type ReactElement } from "react";
import type { EntityId, FreeText, UiText } from "../lib";
import { Button } from "./Button";
import { DataText } from "./Numbers";
import { ScrollArea } from "./ScrollArea";
import { Icon } from "./Icon";

type Entry = {
  readonly id: EntityId;
  readonly title: FreeText | undefined;
  readonly fallback: UiText;
};

type Props = {
  readonly heading: UiText;
  readonly add: UiText;
  readonly entries: readonly Entry[];
  readonly selected: EntityId | undefined;
  readonly availability: "enabled" | "disabled";
  readonly select: (session: EntityId) => void;
  readonly create: () => void;
  readonly feedback: ReactElement | null;
  readonly children: ReactElement;
};

const Row = (props: {
  readonly state: "selected" | "idle";
  readonly entry: Entry;
  readonly select: Props["select"];
}): ReactElement => (
  <button
    type="button"
    aria-current={props.state === "selected" ? "page" : undefined}
    onClick={(): void => {
      props.select(props.entry.id);
    }}
    className={rowClasses[props.state]}
  >
    {props.entry.title === undefined ? (
      props.entry.fallback
    ) : (
      <DataText value={props.entry.title} variant="body-sm" />
    )}
  </button>
);

const rowClasses = {
  selected:
    "block w-full text-left status-sentence px-space-2 py-space-2 text-body-sm rounded-md text-primary bg-primary-soft font-semibold focus-visible:outline-solid focus-visible:outline-focus",
  idle: "block w-full text-left status-sentence px-space-2 py-space-2 text-body-sm rounded-md text-ink hover:bg-surface-sunken focus-visible:outline-solid focus-visible:outline-focus",
} as const;

const SessionList = (
  props: Pick<Props, "entries" | "selected" | "select">,
): ReactElement => (
  <ScrollArea direction="vertical">
    <ul className="flex flex-col gap-space-1 px-space-2">
      {props.entries.map((entry) => (
        <li key={entry.id} className="scroll-flex">
          <Row
            entry={entry}
            state={props.selected === entry.id ? "selected" : "idle"}
            select={props.select}
          />
        </li>
      ))}
    </ul>
  </ScrollArea>
);

const layoutClasses = {
  open: "grid grid-cols-layout flex-1 scroll-flex",
  closed: "flex flex-col flex-1 scroll-flex",
} as const;

const Layout = (props: {
  readonly panel: "open" | "closed";
  readonly children: readonly [ReactElement, ReactElement];
}): ReactElement => (
  <div className={layoutClasses[props.panel]}>{props.children}</div>
);

export const ConversationTree = (props: Props): ReactElement => {
  const [panel, setPanel] = useState<"open" | "closed">("open");
  const listId = useId();
  return (
    <div className="@container flex flex-col flex-1 scroll-flex w-full">
      <Layout panel={panel}>
        <nav
          aria-label={props.heading}
          className="flex flex-col scroll-flex col-span-3 p-space-2 border-b-hairline border-solid border-line bg-surface"
        >
          <div className="flex flex-wrap items-center justify-between gap-space-1">
            <button
              type="button"
              aria-expanded={panel === "open"}
              aria-controls={listId}
              onClick={(): void => {
                setPanel(panel === "open" ? "closed" : "open");
              }}
              className="inline-flex items-center gap-space-1 px-space-2 font-semibold text-ink rounded-md hover:bg-surface-sunken focus-visible:outline-focus"
            >
              <Icon
                name={panel === "open" ? "chevron-down" : "chevron-right"}
              />
              {props.heading}
            </button>
            <Button
              size="control-height-sm"
              onClick={(): void => {
                setPanel("open");
                props.create();
              }}
              disabled={props.availability}
            >
              {props.add}
            </Button>
          </div>
          <div
            id={listId}
            hidden={panel === "closed"}
            className="flex flex-col scroll-flex"
          >
            {panel === "closed" ? null : (
              <SessionList
                entries={props.entries}
                selected={props.selected}
                select={props.select}
              />
            )}
          </div>
          {props.feedback}
        </nav>
        <div className="flex flex-col flex-1 scroll-flex col-span-9">
          {props.children}
        </div>
      </Layout>
    </div>
  );
};
