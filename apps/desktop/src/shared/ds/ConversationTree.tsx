import { useId, useState, type ReactElement } from "react";
import type { EntityId, FreeText, UiText } from "../lib";
import {
  Group,
  Panel,
  Separator,
  usePanelCallbackRef,
} from "react-resizable-panels";
import { PanelLayout } from "./AppShell";
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
    <ul className="flex flex-col gap-space-1 @content-medium:px-space-2">
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

type Visibility = "open" | "closed";

const List = (props: {
  readonly tree: Props;
  readonly panel: Visibility;
  readonly toggle: () => void;
}): ReactElement => {
  const { panel, tree } = props;
  const listId = useId();
  return (
    <nav
      aria-label={tree.heading}
      className="flex flex-col scroll-flex @content-medium:p-space-2 border-b-hairline border-solid border-line bg-surface"
    >
      <div className="flex flex-wrap items-center justify-between gap-space-1">
        <button
          type="button"
          aria-label={tree.heading}
          aria-expanded={panel === "open"}
          aria-controls={listId}
          onClick={(): void => {
            props.toggle();
          }}
          className="inline-flex flex-wrap items-center gap-space-1 @content-medium:px-space-2 font-semibold text-ink rounded-md hover:bg-surface-sunken focus-visible:outline-focus"
        >
          <Icon name={panel === "open" ? "chevron-down" : "chevron-right"} />
          {panel === "open" ? tree.heading : null}
        </button>
        {panel === "open" && (
          <Button
            size="control-height-sm"
            onClick={(): void => {
              tree.create();
            }}
            disabled={tree.availability}
          >
            {tree.add}
          </Button>
        )}
      </div>
      <div
        id={listId}
        hidden={panel === "closed"}
        className="flex flex-col scroll-flex"
      >
        {panel === "closed" ? null : (
          <SessionList
            entries={tree.entries}
            selected={tree.selected}
            select={tree.select}
          />
        )}
      </div>
      {tree.feedback}
    </nav>
  );
};

export const ConversationTree = (props: Props): ReactElement => {
  const [panel, setPanel] = useState<Visibility>("open");
  const [handle, setHandle] = usePanelCallbackRef();
  const list = (
    <List
      tree={props}
      panel={panel}
      toggle={(): void => {
        if (handle !== null) {
          if (panel === "open") handle.collapse();
          else handle.expand();
        }
        setPanel(panel === "open" ? "closed" : "open");
      }}
    />
  );
  const content = (
    <div className="flex flex-col flex-1 scroll-flex">{props.children}</div>
  );
  return (
    <div className="@container flex flex-col flex-1 scroll-flex w-full">
      <PanelLayout
        fallback={
          <>
            {list}
            {content}
          </>
        }
      >
        <Group orientation="horizontal" className="flex flex-1 scroll-flex">
          <Panel
            panelRef={setHandle}
            defaultSize="25"
            minSize="18"
            maxSize="50"
            collapsible
            collapsedSize="5"
            onResize={(size): void => {
              if (size.asPercentage > 0)
                setPanel(handle?.isCollapsed() === true ? "closed" : "open");
            }}
            className="flex flex-col scroll-flex"
          >
            {list}
          </Panel>
          <Separator
            aria-label={props.heading}
            className="flex-none px-space-1 bg-line hover:bg-primary focus-visible:bg-primary focus-visible:outline-focus"
          />
          <Panel minSize="45" className="flex flex-col scroll-flex">
            {content}
          </Panel>
        </Group>
      </PanelLayout>
    </div>
  );
};
