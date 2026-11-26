import type { ReactElement } from "react";
import {
  Card,
  DataText,
  Disclosure,
  Inline,
  Percent,
  Stack,
  Text,
} from "../../../shared/ds";
import type { SettingsView } from "../../../shared/ipc";
import { settingsText, type SettingsMessageKey } from "../messages";
import { AutoStartSwitch } from "./AutoStartSwitch";

type Props = {
  readonly settings: SettingsView;
  readonly onOpenStatus: () => void;
};

type Summary =
  | { readonly kind: "ready"; readonly settings: SettingsView }
  | { readonly kind: "missing" };

export type SummaryScope = "all" | "status-only";

type AdvancedProps = {
  readonly overview: Summary;
  readonly scope: SummaryScope;
  readonly onOpenStatus: () => void;
  readonly editors: readonly ReactElement[];
};

const summaryCards = (
  summary: Summary,
  scope: SummaryScope,
  onOpenStatus: () => void,
): readonly ReactElement[] =>
  summary.kind === "missing"
    ? []
    : [
        ...(scope === "all"
          ? [
              <AiCard key="ai" settings={summary.settings} />,
              <NewsDataCard key="news" settings={summary.settings} />,
              <PrinciplesCard key="principles" settings={summary.settings} />,
            ]
          : []),
        <SystemCard
          key="system"
          settings={summary.settings}
          onOpenStatus={onOpenStatus}
        />,
      ];

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
      <AutoStartSwitch state={props.settings.autoStartAtLogin} />
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

export const AdvancedSettings = (props: AdvancedProps): ReactElement => (
  <Disclosure title={settingsText("advancedTitle", "ko")} defaultOpen="closed">
    <Stack gap="space-4">
      {[
        ...summaryCards(props.overview, props.scope, props.onOpenStatus),
        ...props.editors,
      ]}
    </Stack>
  </Disclosure>
);
