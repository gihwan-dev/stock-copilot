import { useState, type ReactElement } from "react";
import {
  Button,
  ChatInput,
  EmptyState,
  Skeleton,
  Stack,
  Text,
} from "../../../shared/ds";
import type {
  ChatHistory,
  ChatListener,
  ChatScreenContext,
} from "../../../shared/ipc";
import type { EntityId } from "../../../shared/lib";
import { chatText } from "../messages";
import {
  useCancelChat,
  useChatHistory,
  useChatResume,
  useReloadChatHistory,
  useSendChat,
} from "../model/queries";
import {
  applyEvent,
  beginExchange,
  fromHistory,
  resumableTurn,
  settleExchange,
  type Transcript,
} from "../model/transcript";
import { panelState, streamingTurn } from "../model/view";
import { ChatExchangeView } from "./ChatExchangeView";

type Props = { readonly context: ChatScreenContext | undefined };

type Update = (change: (current: Transcript) => Transcript) => void;

const listenerFor =
  (update: Update): ChatListener =>
  (event): void => {
    if (event.kind === "ok")
      update((current) => applyEvent(current, event.value));
  };

const Resume = (props: {
  readonly turn: EntityId;
  readonly update: Update;
}): ReactElement => {
  const state = useChatResume(props.turn, listenerFor(props.update));
  return <Stack gap="space-1">{state === "resuming" ? [] : []}</Stack>;
};

const Exchanges = (props: { readonly transcript: Transcript }): ReactElement =>
  panelState(props.transcript) === "empty" ? (
    <EmptyState title={chatText("chat.emptyTitle", "ko")}>
      {chatText("chat.emptyBody", "ko")}
    </EmptyState>
  ) : (
    <Stack gap="space-4">
      {props.transcript.exchanges.map((exchange) => (
        <ChatExchangeView key={exchange.turn} exchange={exchange} />
      ))}
    </Stack>
  );

const StopButton = (props: {
  readonly live: EntityId | undefined;
}): ReactElement => {
  const stopTurn = useCancelChat();
  const live = props.live;
  return live === undefined ? (
    <Stack gap="space-1">{[]}</Stack>
  ) : (
    <Button
      variant="secondary"
      size="control-height-sm"
      onClick={(): void => {
        stopTurn(live);
      }}
    >
      {chatText("chat.stop", "ko")}
    </Button>
  );
};

const Conversation = (props: {
  readonly history: ChatHistory;
  readonly context: ChatScreenContext | undefined;
}): ReactElement => {
  const [transcript, setTranscript] = useState(() =>
    fromHistory(props.history),
  );
  const update: Update = (change): void => {
    setTranscript(change);
  };
  const send = useSendChat(
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
  const resume = resumableTurn(props.history);
  const live = streamingTurn(transcript);
  return (
    <Stack gap="space-4">
      <Exchanges transcript={transcript} />
      {resume === undefined ? (
        <Stack gap="space-1">{[]}</Stack>
      ) : (
        <Resume turn={resume} update={update} />
      )}
      <StopButton live={live} />
      <ChatInput
        status={live === undefined ? "ready" : "sending"}
        onSend={send}
      />
    </Stack>
  );
};

export const ChatPanel = (props: Props): ReactElement => {
  const view = useChatHistory();
  const reload = useReloadChatHistory();
  switch (view.status) {
    case "loading":
      return (
        <Stack gap="space-2">
          <Text variant="caption" tone="ink-muted">
            {chatText("chat.loading", "ko")}
          </Text>
          <Skeleton variant="text" />
        </Stack>
      );
    case "error":
      return (
        <EmptyState
          title={chatText("chat.historyFailed", "ko")}
          action={{ label: chatText("chat.retry", "ko"), onPress: reload }}
        />
      );
    case "ready":
      return <Conversation history={view.history} context={props.context} />;
  }
};
