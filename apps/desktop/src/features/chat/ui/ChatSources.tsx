import type { ReactElement } from "react";
import { Badge, Inline, Text } from "../../../shared/ds";
import type { ChatSource, ChatSourceKind } from "../../../shared/ipc";
import { chatText } from "../messages";
import { sourceLabel } from "./labels";

const kindOrder: readonly ChatSourceKind[] = [
  "tool",
  "disclosure",
  "news",
  "web",
];

export const ChatSources = (props: {
  readonly sources: readonly ChatSource[];
}): ReactElement => {
  const kinds = kindOrder.filter((kind) =>
    props.sources.some((source) => source.kind === kind),
  );
  return (
    <Inline gap="space-2" wrap="wrap">
      <Text variant="caption" tone="ink-muted">
        {chatText("chat.sourcesTitle", "ko")}
      </Text>
      {kinds.length === 0 ? (
        <Badge tone="alert">{chatText("chat.noSources", "ko")}</Badge>
      ) : (
        kinds.map((kind) => (
          <Badge key={kind} tone={kind === "web" ? "alert" : "neutral"}>
            {sourceLabel(kind)}
          </Badge>
        ))
      )}
    </Inline>
  );
};
