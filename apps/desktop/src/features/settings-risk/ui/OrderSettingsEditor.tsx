import { useState, type ReactElement } from "react";
import {
  Button,
  Card,
  EmptyState,
  Select,
  Skeleton,
  Stack,
  Switch,
  Text,
} from "../../../shared/ds";
import {
  cardExpiries,
  defaultOrderSettings,
  orderCountLimits,
  type OrderSettingsDraft,
  type ScreenSources,
} from "../../../shared/ipc";
import { riskText } from "../messages";
import {
  choicesOf,
  inputOf,
  pick,
  useDraftSource,
  useSave,
} from "../model/risk";
import { SaveNotice } from "./SaveNotice";

type Sources = Pick<ScreenSources, "orderSettings" | "saveOrderSettings">;
type Edit = {
  readonly draft: OrderSettingsDraft;
  readonly change: (draft: OrderSettingsDraft) => void;
};

const expiryChoices = choicesOf(cardExpiries, (value) =>
  riskText(`risk.expiry.${value}`, "ko"),
);
const countChoices = choicesOf(orderCountLimits, (value) =>
  riskText(`risk.count.${value}`, "ko"),
);

const Fields = (props: Edit): ReactElement => (
  <Stack gap="space-4">
    <Switch
      label={riskText("risk.marketOrders", "ko")}
      checked={props.draft.marketOrders}
      onChange={(state): void => {
        props.change({ ...props.draft, marketOrders: state });
      }}
    />
    <Switch
      label={riskText("risk.hoursHorizon", "ko")}
      checked={props.draft.hoursHorizon}
      onChange={(state): void => {
        props.change({ ...props.draft, hoursHorizon: state });
      }}
    />
    <Select
      label={riskText("risk.cardExpiry", "ko")}
      options={expiryChoices}
      value={inputOf(props.draft.cardExpiry)}
      onChange={(value): void => {
        const cardExpiry = pick(cardExpiries, value);
        if (cardExpiry !== undefined)
          props.change({ ...props.draft, cardExpiry });
      }}
    />
    <Select
      label={riskText("risk.orderCount", "ko")}
      options={countChoices}
      value={inputOf(props.draft.orderCountLimit)}
      onChange={(value): void => {
        const orderCountLimit = pick(orderCountLimits, value);
        if (orderCountLimit !== undefined)
          props.change({ ...props.draft, orderCountLimit });
      }}
    />
  </Stack>
);

const Form = (props: {
  readonly initial: OrderSettingsDraft;
  readonly sources: Sources;
  readonly origin: "stored" | "defaults";
}): ReactElement => {
  const [draft, setDraft] = useState<OrderSettingsDraft>(props.initial);
  const save = useSave(props.sources.saveOrderSettings);
  return (
    <Stack gap="space-4">
      {props.origin === "defaults" ? (
        <Text variant="caption" tone="ink-muted">
          {riskText("risk.defaults", "ko")}
        </Text>
      ) : (
        <Stack gap="space-1">{[]}</Stack>
      )}
      <Fields draft={draft} change={setDraft} />
      <Text variant="caption" tone="ink-muted">
        {riskText("risk.confirmNote", "ko")}
      </Text>
      <Button
        variant="primary"
        onClick={(): void => {
          save.submit(draft);
        }}
      >
        {riskText("risk.save", "ko")}
      </Button>
      <SaveNotice notice={save.notice} outcome={save.reason} />
    </Stack>
  );
};

const Body = (props: { readonly sources: Sources }): ReactElement => {
  const source = useDraftSource(
    ["settings", "risk", "order"],
    props.sources.orderSettings,
  );
  switch (source.view.status) {
    case "loading":
      return <Skeleton variant="text" lines={3} />;
    case "error":
      return (
        <EmptyState
          title={riskText("risk.loadFailed", "ko")}
          action={{
            label: riskText("risk.retry", "ko"),
            onPress: source.retry,
          }}
        />
      );
    case "empty":
      return (
        <Form
          initial={defaultOrderSettings}
          sources={props.sources}
          origin="defaults"
        />
      );
    case "ready":
      return (
        <Form
          initial={source.view.data}
          sources={props.sources}
          origin="stored"
        />
      );
  }
};

export const OrderSettingsEditor = (props: {
  readonly sources: Sources;
}): ReactElement => (
  <Card title={riskText("risk.orderTitle", "ko")}>
    <Body sources={props.sources} />
  </Card>
);
