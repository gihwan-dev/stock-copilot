import type { ReactElement } from "react";
import {
  Button,
  ChatInput,
  EmptyState,
  Inline,
  Skeleton,
  Stack,
  Text,
} from "../../../shared/ds";
import {
  mainChatSession,
  type ChatListener,
  type ChatScreenContext,
} from "../../../shared/ipc";
import {
  parseFreeText,
  type EntityId,
  type FreeText,
} from "../../../shared/lib";
import { chatText, type ChatMessageKey } from "../messages";
import {
  useCancelChat,
  useConversation,
  type ConversationState,
  useChatHistory,
  useChatResume,
  useReloadChatHistory,
  useSendChat,
} from "../model/queries";
import {
  applyEvent,
  beginExchange,
  settleExchange,
  type Transcript,
} from "../model/transcript";
import { panelState, streamingTurn } from "../model/view";
import { ChatExchangeView } from "./ChatExchangeView";

export type ChatParts = {
  readonly messages: ReactElement;
  readonly composer: ReactElement | null;
};

type Props = {
  readonly session?: EntityId | undefined;
  readonly context: ChatScreenContext | undefined;
  readonly children: (parts: ChatParts) => ReactElement;
};

type Update = (change: (current: Transcript) => Transcript) => void;

const suggestions: readonly ChatMessageKey[] = [
  "chat.suggest.portfolio",
  "chat.suggest.news",
  "chat.suggest.risk",
];

const listenerFor =
  (update: Update): ChatListener =>
  (event): void => {
    if (event.kind === "ok")
      update((current) => applyEvent(current, event.value));
  };

const Resume = (props: {
  readonly loadId: EntityId;
  readonly turn: EntityId;
  readonly update: Update;
}): ReactElement => {
  const state = useChatResume(
    props.loadId,
    props.turn,
    listenerFor(props.update),
  );
  return <Stack gap="space-1">{state === "resuming" ? [] : []}</Stack>;
};

const Suggestions = (props: {
  readonly onPick: (value: FreeText) => void;
}): ReactElement => (
  <Inline gap="space-2" justify="center" wrap="wrap">
    {suggestions.map((key) => (
      <Button
        key={key}
        variant="secondary"
        size="control-height-sm"
        onClick={(): void => {
          const value = parseFreeText(chatText(key, "ko"));
          if (value.kind === "ok") props.onPick(value.value);
        }}
      >
        {chatText(key, "ko")}
      </Button>
    ))}
  </Inline>
);

const Exchanges = (props: {
  readonly transcript: Transcript;
  readonly onPick: (value: FreeText) => void;
}): ReactElement =>
  panelState(props.transcript) === "empty" ? (
    <Stack gap="space-2">
      <EmptyState
        icon="message-square"
        title={chatText("chat.emptyTitle", "ko")}
      >
        {chatText("chat.emptyBody", "ko")}
      </EmptyState>
      <Suggestions onPick={props.onPick} />
    </Stack>
  ) : (
    <Stack gap="space-6">
      {props.transcript.exchanges.map((exchange) => (
        <ChatExchangeView key={exchange.turn} exchange={exchange} />
      ))}
    </Stack>
  );

const StopButton = (props: { readonly live: EntityId }): ReactElement => {
  const stopTurn = useCancelChat();
  return (
    <Button
      variant="secondary"
      size="control-height-sm"
      onClick={(): void => {
        stopTurn(props.live);
      }}
    >
      {chatText("chat.stop", "ko")}
    </Button>
  );
};

const Conversation = (props: {
  readonly conversation: ConversationState;
  readonly context: ChatScreenContext | undefined;
  readonly render: (parts: ChatParts) => ReactElement;
}): ReactElement => {
  const { transcript, resume } = props.conversation;
  const { draft, update, setDraft } = useConversation(props.conversation);
  const send = useSendChat(
    props.conversation.session,
    {
      begin: (turn, message): void => {
        update((current) => beginExchange(current, turn, message));
      },
      listener: listenerFor(update),
      settle: (turn, status): void => {
        update((current) => settleExchange(current, turn, status));
      },
    },
    props.context,
  );
  const live = streamingTurn(transcript);
  const parts = props.render({
    messages: <Exchanges transcript={transcript} onPick={setDraft} />,
    composer: (
      <ChatInput
        secondaryAction={
          live === undefined ? undefined : <StopButton live={live} />
        }
        layout="compact"
        draft={draft}
        onDraftChange={setDraft}
        status={live === undefined ? "ready" : "sending"}
        onSend={send}
      />
    ),
  });
  return resume === undefined ? (
    parts
  ) : (
    <>
      {parts}
      <Resume
        loadId={props.conversation.loadId}
        turn={resume}
        update={update}
      />
    </>
  );
};

const Loading = (): ReactElement => (
  <Stack gap="space-2">
    <Text variant="caption" tone="ink-muted">
      {chatText("chat.loading", "ko")}
    </Text>
    <Skeleton variant="text" />
  </Stack>
);

export const ChatSession = (props: Props): ReactElement => {
  const main = mainChatSession();
  const session =
    props.session ?? (main.kind === "ok" ? main.value : undefined);
  const view = useChatHistory(session);
  const reload = useReloadChatHistory();
  switch (view.status) {
    case "loading":
      return props.children({ messages: <Loading />, composer: null });
    case "error":
      return props.children({
        messages: (
          <EmptyState
            title={chatText("chat.historyFailed", "ko")}
            action={{ label: chatText("chat.retry", "ko"), onPress: reload }}
          />
        ),
        composer: null,
      });
    case "ready":
      return (
        <Conversation
          key={view.conversation.loadId}
          conversation={view.conversation}
          context={props.context}
          render={props.children}
        />
      );
  }
};

export const ChatPanel = (props: {
  readonly session?: EntityId | undefined;
  readonly children?: (parts: ChatParts) => ReactElement;
  readonly context: ChatScreenContext | undefined;
}): ReactElement => (
  <ChatSession context={props.context} session={props.session}>
    {props.children ??
      ((parts): ReactElement => (
        <Stack gap="space-4">
          {parts.messages}
          {parts.composer ?? <Stack gap="space-1">{[]}</Stack>}
        </Stack>
      ))}
  </ChatSession>
);
