import type { ReactElement } from "react";
import { Button, Card, Stack, Text } from "../../../shared/ds";
import type { ChatCardRef } from "../../../shared/ipc";
import { chatText } from "../messages";
import { cardLabel } from "./labels";

export const SignalCard = (props: {
  readonly card: ChatCardRef;
}): ReactElement => (
  <Card title={cardLabel(props.card.kind)}>
    <Stack gap="space-2">
      <Text variant="body-sm">{chatText("chat.signalBody", "ko")}</Text>
      <Button variant="primary" disabled="disabled">
        {chatText("chat.approve", "ko")}
      </Button>
      <Text variant="caption" tone="ink-muted">
        {chatText("chat.approveUnavailable", "ko")}
      </Text>
    </Stack>
  </Card>
);

const InfoCard = (props: { readonly card: ChatCardRef }): ReactElement => (
  <Card title={cardLabel(props.card.kind)}>
    <Text variant="body-sm">{chatText("chat.cardBody", "ko")}</Text>
  </Card>
);

export const ChatCardView = (props: {
  readonly card: ChatCardRef;
}): ReactElement => {
  switch (props.card.kind) {
    case "signal":
    case "proposal":
      return <SignalCard card={props.card} />;
    case "holding":
    case "quote":
    case "briefing":
    case "disclosure":
      return <InfoCard card={props.card} />;
  }
};
