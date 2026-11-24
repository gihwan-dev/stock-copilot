import type { ReactElement } from "react";
import { Badge, Inline, Markdown, Stack, Text } from "../../../shared/ds";
import type { ExternalUrl, UiText } from "../../../shared/lib";
import { useOpenLink } from "../model/queries";
import { chatText } from "../messages";
import type { AssistantReply, ChatExchange } from "../model/transcript";
import { linkVerdict } from "../model/view";
import { ChatCardView } from "./ChatCardView";
import { ChatSources } from "./ChatSources";
import { failureLabel, toolLabel } from "./labels";

const denyLinks = (): "deny" => "deny";

const replyNotice = (reply: AssistantReply): UiText | undefined => {
  switch (reply.phase) {
    case "streaming":
      return reply.tool === undefined
        ? chatText("chat.writing", "ko")
        : toolLabel(reply.tool);
    case "done":
      return undefined;
    case "cancelled":
      return chatText("chat.cancelled", "ko");
    case "interrupted":
      return chatText("chat.interrupted", "ko");
    case "rate-limited":
      return chatText("chat.rateLimited", "ko");
    case "failed":
      return failureLabel(reply.failure);
  }
};

const Notice = (props: { readonly reply: AssistantReply }): ReactElement => {
  const notice = replyNotice(props.reply);
  return notice === undefined ? (
    <Stack gap="space-1">{[]}</Stack>
  ) : (
    <Text
      variant="caption"
      tone={props.reply.phase === "streaming" ? "ink-muted" : "alert"}
    >
      {notice}
    </Text>
  );
};

const Reply = (props: {
  readonly reply: AssistantReply;
  readonly openLink: (url: ExternalUrl) => void;
}): ReactElement => (
  <Stack gap="space-2">
    <Text variant="label" tone="primary">
      {chatText("chat.assistant", "ko")}
    </Text>
    {props.reply.researched === "researched" ? (
      <Text variant="caption" tone="ink-muted">
        {chatText("chat.webResearched", "ko")}
      </Text>
    ) : (
      <Stack gap="space-1">{[]}</Stack>
    )}
    <Markdown
      content={props.reply.text}
      linkAllowed={(url) => linkVerdict(props.reply, url)}
      onOpenLink={props.openLink}
    />
    <Stack gap="space-2">
      {props.reply.cards.map((card) => (
        <ChatCardView key={`${card.kind}-${card.id}`} card={card} />
      ))}
    </Stack>
    {props.reply.numbers === "unverified" ? (
      <Inline gap="space-2" wrap="wrap">
        <Badge tone="alert">{chatText("chat.unverified", "ko")}</Badge>
        <Text variant="caption" tone="ink-muted">
          {chatText("chat.unverifiedBody", "ko")}
        </Text>
      </Inline>
    ) : (
      <Stack gap="space-1">{[]}</Stack>
    )}
    {props.reply.phase === "done" ? (
      <ChatSources sources={props.reply.sources} />
    ) : (
      <Stack gap="space-1">{[]}</Stack>
    )}
    <Notice reply={props.reply} />
  </Stack>
);

export const ChatExchangeView = (props: {
  readonly exchange: ChatExchange;
}): ReactElement => {
  const openLink = useOpenLink();
  return (
    <Stack gap="space-3">
      <Stack gap="space-1">
        <Text variant="label" tone="ink-muted">
          {chatText("chat.you", "ko")}
        </Text>
        <Markdown
          content={props.exchange.question}
          linkAllowed={denyLinks}
          onOpenLink={openLink}
        />
      </Stack>
      <Reply reply={props.exchange.reply} openLink={openLink} />
    </Stack>
  );
};
