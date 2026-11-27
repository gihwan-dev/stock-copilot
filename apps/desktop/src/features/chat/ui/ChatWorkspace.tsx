import type { ReactElement } from "react";
import { Button, ConversationTree, Stack, Text } from "../../../shared/ds";
import {
  parseFreeText,
  type EntityId,
  type FreeText,
} from "../../../shared/lib";
import { chatText } from "../messages";
import { useSessionDirectory, type Directory } from "../model/sessions";
import { ChatPanel, type ChatParts } from "./ChatPanel";

const feedbackFor = (directory: Directory): ReactElement | null => {
  if (directory.state === "failed")
    return (
      <Stack gap="space-1">
        <Text variant="caption">{chatText("chat.listFailed", "ko")}</Text>
        <Button size="control-height-sm" onClick={directory.retry}>
          {chatText("chat.retry", "ko")}
        </Button>
      </Stack>
    );
  return directory.creation === "failed" ? (
    <Text variant="caption">{chatText("chat.createFailed", "ko")}</Text>
  ) : null;
};

const numberedTitle = (
  id: EntityId,
  positions: ReadonlyMap<EntityId, FreeText>,
): FreeText | undefined => (positions.size > 1 ? positions.get(id) : undefined);

const untitledPositions = (
  directory: Directory,
): ReadonlyMap<EntityId, FreeText> => {
  const untitled = directory.entries
    .filter(
      (item) => item.title === undefined && item.session !== directory.main,
    )
    .map((item) => item.session)
    .toSorted();
  return new Map(
    untitled.flatMap((id, index) => {
      const label = parseFreeText(
        `${chatText("chat.untitled", "ko")} ${(index + 1).toString()}`,
      );
      return label.kind === "ok" ? [[id, label.value] as const] : [];
    }),
  );
};

export const ChatWorkspace = (props: {
  readonly children: (parts: ChatParts) => ReactElement;
}): ReactElement => {
  const directory = useSessionDirectory();
  const positions = untitledPositions(directory);
  return (
    <ConversationTree
      heading={chatText("chat.directory", "ko")}
      add={chatText("chat.create", "ko")}
      entries={directory.entries.map((entry) => ({
        id: entry.session,
        title: entry.title ?? numberedTitle(entry.session, positions),
        fallback: chatText(
          entry.session === directory.main
            ? "chat.defaultSession"
            : "chat.untitled",
          "ko",
        ),
      }))}
      selected={directory.selected}
      availability={
        directory.creation === "pending" || directory.state !== "ready"
          ? "disabled"
          : "enabled"
      }
      select={directory.select}
      create={directory.create}
      feedback={feedbackFor(directory)}
    >
      {directory.selected === undefined ? (
        props.children({
          messages: (
            <Text>
              {chatText(
                directory.state === "failed"
                  ? "chat.listFailed"
                  : "chat.loading",
                "ko",
              )}
            </Text>
          ),
          composer: null,
        })
      ) : (
        <ChatPanel session={directory.selected} context={undefined}>
          {props.children}
        </ChatPanel>
      )}
    </ConversationTree>
  );
};
