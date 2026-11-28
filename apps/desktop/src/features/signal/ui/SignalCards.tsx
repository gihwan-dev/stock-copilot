import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState, type ReactElement } from "react";
import {
  Button,
  Card,
  DataText,
  Divider,
  Disclosure,
  Inline,
  Skeleton,
  Stack,
  Text,
  TextField,
} from "../../../shared/ds";
import type {
  OrderRequestOutcome,
  ScreenSources,
  SignalCardView,
} from "../../../shared/ipc";
import {
  emptyInputValue,
  queryKeys,
  type EntityId,
  type InputValue,
} from "../../../shared/lib";
import { detach } from "../../../shared/platform";
import { signalText } from "../messages";
import { OutcomeNotice } from "./OutcomeNotice";
import { cardAction, useSignals } from "../model/signals";
import { Facts, KeyFacts, Reasons } from "./SignalDetails";

export type SignalHandlers = {
  readonly outcomeOf: (card: EntityId) => OrderRequestOutcome | undefined;
  readonly onRequestApproval: (card: EntityId, proposal: EntityId) => void;
  readonly onSkip: (card: EntityId, reason: InputValue) => void;
};

const Actions = (props: {
  readonly card: SignalCardView;
  readonly handlers: SignalHandlers;
}): ReactElement => {
  const [reason, setReason] = useState<InputValue>(emptyInputValue);
  const action = cardAction(props.card);
  const skip = (
    <Disclosure title={signalText("skip", "ko")}>
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
          {signalText("confirmSkip", "ko")}
        </Button>
      </Stack>
    </Disclosure>
  );
  switch (action.kind) {
    case "order":
      return (
        <Stack gap="space-2">
          <Button
            variant="primary"
            onClick={(): void => {
              props.handlers.onRequestApproval(props.card.id, action.proposal);
            }}
          >
            {signalText("order", "ko")}
          </Button>
          {skip}
        </Stack>
      );
    case "market-closed":
      return <Stack gap="space-2">{[]}</Stack>;
    case "switch-off":
      return (
        <Stack gap="space-2">
          <Text variant="body-sm" tone="ink-muted">
            {signalText("switchOff", "ko")}
          </Text>
          {skip}
        </Stack>
      );
    case "no-order":
    case "zero-quantity":
      return skip;
  }
};

const SignalCard = (props: {
  readonly card: SignalCardView;
  readonly handlers: SignalHandlers;
}): ReactElement => {
  const outcome = props.handlers.outcomeOf(props.card.id);
  return (
    <Stack gap="space-3">
      <Text variant="label">
        {props.card.market === "closed"
          ? signalText("marketClosed", "ko")
          : signalText(props.card.direction, "ko")}
      </Text>
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
          []
        )}
        <KeyFacts card={props.card} />
        <Reasons card={props.card} />
        <Actions card={props.card} handlers={props.handlers} />
        {outcome === undefined ? [] : <OutcomeNotice outcome={outcome} />}
        <Facts card={props.card} />
      </Stack>
    </Stack>
  );
};

type SignalActions = Pick<
  ScreenSources,
  "signals" | "requestOrderApproval" | "skipSignal"
>;

const useHandlers = (sources: SignalActions): SignalHandlers => {
  const client = useQueryClient();
  const refresh = (): void => {
    detach({ kind: "invalidate-queries", client, keys: [queryKeys.signals] });
  };
  const [outcomes, setOutcomes] = useState<
    ReadonlyMap<EntityId, OrderRequestOutcome>
  >(() => new Map());
  const approval = useMutation({
    mutationFn: (input: {
      readonly card: EntityId;
      readonly proposal: EntityId;
    }) => sources.requestOrderApproval(input.proposal),
    onSuccess: (result, input): void => {
      const outcome: OrderRequestOutcome =
        result.kind === "ok" ? result.value : "other";
      setOutcomes((previous) => new Map([...previous, [input.card, outcome]]));
    },
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
    outcomeOf: (card) => outcomes.get(card),
    onRequestApproval: (card, proposal): void => {
      approval.mutate({ card, proposal });
    },
    onSkip: (card, reason): void => {
      skip.mutate({ card, reason });
    },
  };
};

export const SignalCards = (props: {
  readonly sources: SignalActions;
  readonly footer: ReactElement;
}): ReactElement => {
  const handlers = useHandlers(props.sources);
  return (
    <Card title={signalText("title", "ko")} layout="flow">
      <SignalList load={props.sources.signals} handlers={handlers} />
      {props.footer}
    </Card>
  );
};

const SignalItems = (props: {
  readonly cards: readonly SignalCardView[];
  readonly handlers: SignalHandlers;
}): ReactElement => (
  <Stack gap="space-4">
    {props.cards.flatMap((card, index): readonly ReactElement[] => [
      ...(index === 0 ? [] : [<Divider key={`${card.id}-divider`} />]),
      <SignalCard key={card.id} card={card} handlers={props.handlers} />,
    ])}
  </Stack>
);

const SignalList = (props: {
  readonly load: ScreenSources["signals"];
  readonly handlers: SignalHandlers;
}): ReactElement => {
  const signals = useSignals(props.load);
  switch (signals.view.status) {
    case "loading":
      return (
        <Stack gap="space-2">
          <Text variant="body-sm" tone="ink-muted">
            {signalText("loading", "ko")}
          </Text>
          <Skeleton variant="text" lines={3} />
        </Stack>
      );
    case "empty":
      return (
        <Stack gap="space-1">
          <Text variant="body-sm">{signalText("emptyTitle", "ko")}</Text>
          <Text variant="body-sm" tone="ink-muted">
            {signalText("emptyBody", "ko")}
          </Text>
        </Stack>
      );
    case "error":
      return (
        <Stack gap="space-2">
          <Text variant="body-sm">{signalText("errorTitle", "ko")}</Text>
          <Inline gap="space-2" align="center">
            <Button variant="ghost" onClick={signals.retry}>
              {signalText("retry", "ko")}
            </Button>
          </Inline>
        </Stack>
      );
    case "ready":
      return (
        <SignalItems
          cards={signals.view.data.cards}
          handlers={props.handlers}
        />
      );
  }
};
