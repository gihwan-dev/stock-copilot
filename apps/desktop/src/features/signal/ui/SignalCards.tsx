import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState, type ReactElement } from "react";
import {
  Button,
  Card,
  DataText,
  Inline,
  Skeleton,
  Stack,
  Text,
  TextField,
} from "../../../shared/ds";
import type { ScreenSources, SignalCardView } from "../../../shared/ipc";
import {
  emptyInputValue,
  queryKeys,
  type EntityId,
  type InputValue,
} from "../../../shared/lib";
import { detach } from "../../../shared/platform";
import { signalText } from "../messages";
import { cardAction, useSignals } from "../model/signals";
import { Facts, KeyFacts, Reasons } from "./SignalDetails";

export type SignalHandlers = {
  readonly onRequestApproval: (proposal: EntityId) => void;
  readonly onSkip: (card: EntityId, reason: InputValue) => void;
};

const Actions = (props: {
  readonly card: SignalCardView;
  readonly handlers: SignalHandlers;
}): ReactElement => {
  const [reason, setReason] = useState<InputValue>(emptyInputValue);
  const action = cardAction(props.card);
  const skip = (
    <Stack gap="space-2">
      <TextField
        label={signalText("skipReason", "ko")}
        hint={signalText("skipReasonHint", "ko")}
        value={reason}
        onChange={setReason}
      />
      <Button
        variant="ghost"
        onClick={(): void => {
          props.handlers.onSkip(props.card.id, reason);
        }}
      >
        {signalText("skip", "ko")}
      </Button>
    </Stack>
  );
  switch (action.kind) {
    case "order":
      return (
        <Stack gap="space-2">
          <Button
            variant="primary"
            onClick={(): void => {
              props.handlers.onRequestApproval(action.proposal);
            }}
          >
            {signalText("order", "ko")}
          </Button>
          {skip}
        </Stack>
      );
    case "market-closed":
      return <Stack gap="space-2">{[]}</Stack>;
    case "no-order":
    case "zero-quantity":
      return skip;
  }
};

const SignalCard = (props: {
  readonly card: SignalCardView;
  readonly handlers: SignalHandlers;
}): ReactElement => (
  <Card
    title={
      props.card.market === "closed"
        ? signalText("marketClosed", "ko")
        : signalText(props.card.direction, "ko")
    }
  >
    <Stack gap="space-3">
      <Inline gap="space-2" align="center" wrap="wrap">
        <DataText value={props.card.name} />
        <DataText value={props.card.ticker} variant="code" />
      </Inline>
      {props.card.market === "closed" ? (
        <Text variant="body-sm" tone="ink-muted">
          {signalText("marketClosedBody", "ko")}
        </Text>
      ) : (
        <Stack gap="space-1">{[]}</Stack>
      )}
      <KeyFacts card={props.card} />
      <Reasons card={props.card} />
      <Actions card={props.card} handlers={props.handlers} />
      <Facts card={props.card} />
    </Stack>
  </Card>
);

type SignalActions = Pick<
  ScreenSources,
  "signals" | "requestOrderApproval" | "skipSignal"
>;

const useHandlers = (sources: SignalActions): SignalHandlers => {
  const client = useQueryClient();
  const refresh = (): void => {
    detach({ kind: "invalidate-queries", client, keys: [queryKeys.signals] });
  };
  const approval = useMutation({
    mutationFn: (proposal: EntityId) => sources.requestOrderApproval(proposal),
    onSettled: refresh,
  });
  const skip = useMutation({
    mutationFn: (input: {
      readonly card: EntityId;
      readonly reason: InputValue;
    }) => sources.skipSignal(input.card, input.reason),
    onSettled: refresh,
  });
  return {
    onRequestApproval: (proposal): void => {
      approval.mutate(proposal);
    },
    onSkip: (card, reason): void => {
      skip.mutate({ card, reason });
    },
  };
};

export const SignalCards = (props: {
  readonly sources: SignalActions;
}): ReactElement => {
  const handlers = useHandlers(props.sources);
  return <SignalList load={props.sources.signals} handlers={handlers} />;
};

const SignalList = (props: {
  readonly load: ScreenSources["signals"];
  readonly handlers: SignalHandlers;
}): ReactElement => {
  const signals = useSignals(props.load);
  switch (signals.view.status) {
    case "loading":
      return (
        <Card title={signalText("loading", "ko")}>
          <Skeleton variant="text" lines={3} />
        </Card>
      );
    case "empty":
      return (
        <Card title={signalText("emptyTitle", "ko")}>
          <Text variant="body-sm" tone="ink-muted">
            {signalText("emptyBody", "ko")}
          </Text>
        </Card>
      );
    case "error":
      return (
        <Card
          title={signalText("errorTitle", "ko")}
          actions={{ label: signalText("retry", "ko"), onPress: signals.retry }}
        >
          <Stack gap="space-1">{[]}</Stack>
        </Card>
      );
    case "ready":
      return (
        <Stack gap="space-4">
          {signals.view.data.cards.map((card): ReactElement => (
            <SignalCard key={card.id} card={card} handlers={props.handlers} />
          ))}
        </Stack>
      );
  }
};
