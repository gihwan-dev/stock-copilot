import { useState, type ReactElement } from "react";
import {
  Badge,
  Button,
  Inline,
  Markdown,
  Skeleton,
  Stack,
  Text,
} from "../../../shared/ds";
import type { ExternalUrl, UiText } from "../../../shared/lib";
import { useOpenLink } from "../model/queries";
import { chatText } from "../messages";
import type { AssistantReply, ChatExchange } from "../model/transcript";
import { linkVerdict, replyFold } from "../model/view";
import { ChatCardView } from "./ChatCardView";
import { ChatSources } from "./ChatSources";
import { changeLabel, failureLabel, toolLabel } from "./labels";

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

const numberMarks = {
  computed: {
    tone: "neutral",
    label: chatText("chat.computed", "ko"),
    body: chatText("chat.computedBody", "ko"),
  },
  unverified: {
    tone: "alert",
    label: chatText("chat.unverified", "ko"),
    body: chatText("chat.unverifiedBody", "ko"),
  },
} as const;

const NumberMark = (props: {
  readonly numbers: AssistantReply["numbers"];
}): ReactElement => {
  if (props.numbers !== "computed" && props.numbers !== "unverified")
    return <Stack gap="space-1">{[]}</Stack>;
  const mark = numberMarks[props.numbers];
  return (
    <Inline gap="space-2" wrap="wrap">
      <Badge tone={mark.tone}>{mark.label}</Badge>
      <Text variant="caption" tone="ink-muted">
        {mark.body}
      </Text>
    </Inline>
  );
};

const ReplyText = (props: {
  readonly reply: AssistantReply;
  readonly openLink: (url: ExternalUrl) => void;
}): ReactElement => {
  const [open, setOpen] = useState<"open" | "closed">("closed");
  const fold =
    props.reply.phase === "streaming"
      ? ({ kind: "whole" } as const)
      : replyFold(props.reply.text);
  const shown =
    fold.kind === "folded" && open === "closed" ? fold.head : props.reply.text;
  return (
    <Stack gap="space-1">
      <Markdown
        flavor="gfm"
        content={shown}
        linkAllowed={(url) => linkVerdict(props.reply, url)}
        onOpenLink={props.openLink}
      />
      {fold.kind === "folded" ? (
        <Inline gap="space-1">
          <Button
            variant="ghost"
            size="control-height-sm"
            onClick={() => {
              setOpen(open === "closed" ? "open" : "closed");
            }}
          >
            {chatText(open === "closed" ? "chat.more" : "chat.less", "ko")}
          </Button>
        </Inline>
      ) : (
        <Stack gap="space-1">{[]}</Stack>
      )}
    </Stack>
  );
};

const Reply = (props: {
  readonly reply: AssistantReply;
  readonly openLink: (url: ExternalUrl) => void;
}): ReactElement => (
  <Stack gap="space-2">
    <Text variant="caption" tone="ink-subtle">
      {chatText("chat.assistant", "ko")}
    </Text>
    {props.reply.researched === "researched" ? (
      <Text variant="caption" tone="ink-muted">
        {chatText("chat.webResearched", "ko")}
      </Text>
    ) : (
      <Stack gap="space-1">{[]}</Stack>
    )}
    <Stack gap="space-1">
      {props.reply.changes.map((change) => (
        <Text key={change} variant="caption" tone="ink-muted">
          {changeLabel(change)}
        </Text>
      ))}
    </Stack>
    {props.reply.phase === "streaming" && props.reply.text === "" ? (
      <Skeleton variant="text" />
    ) : (
      <ReplyText reply={props.reply} openLink={props.openLink} />
    )}
    <Stack gap="space-2">
      {props.reply.cards.map((card) => (
        <ChatCardView key={`${card.kind}-${card.id}`} card={card} />
      ))}
    </Stack>
    <NumberMark numbers={props.reply.numbers} />
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
      <Inline justify="end">
        <Markdown
          appearance="question"
          content={props.exchange.question}
          linkAllowed={denyLinks}
          onOpenLink={openLink}
        />
      </Inline>
      <Reply reply={props.exchange.reply} openLink={openLink} />
    </Stack>
  );
};
