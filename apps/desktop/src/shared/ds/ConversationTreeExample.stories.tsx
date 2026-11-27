import { useState, type ReactElement } from "react";
import {
  parseEntityId,
  parseFreeText,
  text,
  type EntityId,
  type FreeText,
  type UiText,
} from "../lib";
import { ConversationTree } from "./ConversationTree";
import { Text } from "./Text";
type ExampleProps = {
  readonly titles: readonly UiText[];
  readonly suffix?: UiText;
};

type Entry = {
  readonly id: EntityId;
  readonly title: FreeText;
  readonly fallback: UiText;
  readonly topic: UiText;
};

const initialEntries = (props: ExampleProps): readonly Entry[] =>
  props.titles.flatMap((value, index) => {
    const id = parseEntityId(`story-chat-${index.toString()}`);
    const title = parseFreeText(
      props.suffix === undefined
        ? value
        : `${value} · ${(index + 1).toString()} · ${props.suffix}`,
    );
    return id.kind === "ok" && title.kind === "ok"
      ? [
          {
            id: id.value,
            topic:
              props.suffix === undefined
                ? value
                : text("catalog.marketChat", "ko"),
            title: title.value,
            fallback: text("catalog.newChat", "ko"),
          },
        ]
      : [];
  });

export const ConversationTreeExample = (props: ExampleProps): ReactElement => {
  const [entries, setEntries] = useState(() => initialEntries(props));
  const [selected, setSelected] = useState<EntityId | undefined>(
    entries[0]?.id,
  );
  const current = entries.find((entry) => entry.id === selected);
  return (
    <ConversationTree
      heading={text("nav.chat", "ko")}
      add={text("catalog.newChat", "ko")}
      entries={entries}
      selected={selected}
      availability="enabled"
      select={setSelected}
      create={(): void => {
        const id = parseEntityId(`story-chat-${entries.length.toString()}`);
        const title = parseFreeText(text("catalog.newChat", "ko"));
        if (id.kind === "ok" && title.kind === "ok") {
          setEntries([
            ...entries,
            {
              id: id.value,
              topic: text("catalog.newChat", "ko"),
              title: title.value,
              fallback: text("catalog.newChat", "ko"),
            },
          ]);
          setSelected(id.value);
        }
      }}
      feedback={null}
    >
      <div className="flex flex-col flex-1 scroll-flex p-space-2">
        <Text>{current?.topic ?? text("nav.chat", "ko")}</Text>
      </div>
    </ConversationTree>
  );
};
