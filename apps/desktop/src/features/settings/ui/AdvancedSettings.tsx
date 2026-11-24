import type { ReactElement } from "react";
import {
  Badge,
  Card,
  DataText,
  Disclosure,
  Inline,
  Percent,
  Stack,
  Text,
} from "../../../shared/ds";
import type { SettingsView, ToggleState } from "../../../shared/ipc";
import { settingsText, type SettingsMessageKey } from "../messages";

type Props = {
  readonly settings: SettingsView;
  readonly onOpenStatus: () => void;
};

const toggleMessage = (state: ToggleState): SettingsMessageKey =>
  state === "on" ? "stateOn" : "stateOff";

const OptionalKeyRow = (props: {
  readonly labelKey: SettingsMessageKey;
  readonly configured: "configured" | "unconfigured";
}): ReactElement => (
  <Inline gap="space-2" align="center" wrap="wrap">
    <Text variant="label">{settingsText(props.labelKey, "ko")}</Text>
    <Text variant="body-sm" tone="ink-muted">
      {settingsText(
        props.configured === "configured"
          ? "optionalKeyConfigured"
          : "optionalKeyMissing",
        "ko",
      )}
    </Text>
  </Inline>
);

const ToggleSummaryRow = (props: {
  readonly labelKey: SettingsMessageKey;
  readonly state: ToggleState;
}): ReactElement => (
  <Inline gap="space-2" align="center" wrap="wrap">
    <Text variant="label">{settingsText(props.labelKey, "ko")}</Text>
    <Badge tone={props.state === "on" ? "primary" : "neutral"}>
      {settingsText(toggleMessage(props.state), "ko")}
    </Badge>
  </Inline>
);

const AiCard = (props: { readonly settings: SettingsView }): ReactElement => (
  <Card title={settingsText("advancedAiTitle", "ko")}>
    <Stack gap="space-3">
      <Text variant="body-sm">{settingsText("advancedAiModels", "ko")}</Text>
      <Inline gap="space-2" align="center" wrap="wrap">
        <Text variant="label">
          {settingsText("advancedAiBudgetLabel", "ko")}
        </Text>
        <Percent value={props.settings.aiBudget} />
      </Inline>
      <Text variant="caption" tone="ink-muted">
        {settingsText("advancedAiPrivacy", "ko")}
      </Text>
    </Stack>
  </Card>
);

const NewsDataCard = (props: {
  readonly settings: SettingsView;
}): ReactElement => (
  <Card title={settingsText("advancedNewsTitle", "ko")}>
    <Stack gap="space-3">
      <Text variant="body-sm">
        {settingsText("advancedNewsBodyRead", "ko")}
      </Text>
      <OptionalKeyRow
        labelKey="dartKeyLabel"
        configured={props.settings.dartKey}
      />
      <OptionalKeyRow
        labelKey="fredKeyLabel"
        configured={props.settings.fredKey}
      />
    </Stack>
  </Card>
);

const OrderRulesCard = (props: {
  readonly settings: SettingsView;
}): ReactElement => (
  <Card title={settingsText("advancedOrderTitle", "ko")}>
    <Stack gap="space-3">
      <ToggleSummaryRow
        labelKey="marketOrderLabel"
        state={props.settings.allowMarketOrders}
      />
      <ToggleSummaryRow
        labelKey="hoursHorizonLabel"
        state={props.settings.allowHoursHorizon}
      />
      <Inline gap="space-2" align="center" wrap="wrap">
        <Text variant="label">{settingsText("cardExpiryLabel", "ko")}</Text>
        <DataText value={props.settings.cardExpiryMinutes} variant="body-sm" />
      </Inline>
    </Stack>
  </Card>
);

const PrinciplesCard = (props: {
  readonly settings: SettingsView;
}): ReactElement => (
  <Card title={settingsText("advancedPrinciplesTitle", "ko")}>
    <Inline gap="space-2" align="center" wrap="wrap">
      <Text variant="label">
        {settingsText("concentrationRuleLabel", "ko")}
      </Text>
      <DataText
        value={props.settings.concentrationThreshold}
        variant="body-sm"
      />
    </Inline>
  </Card>
);

const SystemCard = (props: Props): ReactElement => (
  <Card
    title={settingsText("advancedSystemTitle", "ko")}
    actions={{
      label: settingsText("openStatusButton", "ko"),
      onPress: props.onOpenStatus,
    }}
  >
    <Stack gap="space-3">
      <ToggleSummaryRow
        labelKey="autoStartLabel"
        state={props.settings.autoStartAtLogin}
      />
      <Inline gap="space-2" align="center" wrap="wrap">
        <Text variant="label">
          {settingsText("accountSelectionLabel", "ko")}
        </Text>
        <DataText value={props.settings.accountCount} variant="body-sm" />
      </Inline>
      <Text variant="caption" tone="ink-muted">
        {settingsText("accountSelectionHint", "ko")}
      </Text>
      <Text variant="caption" tone="ink-subtle">
        {settingsText("dataToolsHint", "ko")}
      </Text>
    </Stack>
  </Card>
);

export const AdvancedSettings = (props: Props): ReactElement => (
  <Disclosure title={settingsText("advancedTitle", "ko")} defaultOpen="closed">
    <Stack gap="space-4">
      <AiCard settings={props.settings} />
      <NewsDataCard settings={props.settings} />
      <OrderRulesCard settings={props.settings} />
      <PrinciplesCard settings={props.settings} />
      <SystemCard settings={props.settings} onOpenStatus={props.onOpenStatus} />
    </Stack>
  </Disclosure>
);
