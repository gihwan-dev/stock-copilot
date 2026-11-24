import type { ReactElement } from "react";
import {
  Badge,
  Card,
  DataText,
  EmptyState,
  Inline,
  Page,
  Percent,
  Section,
  Skeleton,
  Stack,
  Switch,
  Text,
} from "../../../shared/ds";
import type {
  HorizonOption,
  OrderPermissionState,
  RiskAppetiteOption,
  SettingsView,
} from "../../../shared/ipc";
import { settingsText, type SettingsMessageKey } from "../messages";
import {
  useOrderPermissionSwitch,
  useSettingsQuery,
  type OrderSwitchFeedback,
  type SettingsQueryState,
} from "../model/settings";
import { AdvancedSettings } from "./AdvancedSettings";

type Props = {
  readonly onBack: () => void;
  readonly onOpenStatus: () => void;
  readonly system: ReactElement | null;
};

const horizonKeys = {
  hours: "horizonHours",
  "few-days": "horizonFewDays",
  "weeks-to-months": "horizonWeeksToMonths",
  "half-year-or-more": "horizonHalfYearOrMore",
} as const satisfies Record<HorizonOption, SettingsMessageKey>;

const riskKeys = {
  conservative: "riskConservative",
  neutral: "riskNeutral",
  aggressive: "riskAggressive",
} as const satisfies Record<RiskAppetiteOption, SettingsMessageKey>;

const feedbackKey = (
  feedback: OrderSwitchFeedback,
): SettingsMessageKey | undefined => {
  switch (feedback) {
    case "idle":
    case "submitting":
      return undefined;
    case "changed":
      return "orderChanged";
    case "denied":
      return "orderDenied";
    case "unavailable":
    case "failed":
      return "orderUnavailable";
  }
};

const TossConnectionCard = (props: {
  readonly settings: SettingsView;
}): ReactElement => (
  <Card title={settingsText("tossTitle", "ko")}>
    <Stack gap="space-2">
      <Inline gap="space-2" align="center" wrap="wrap">
        <Badge
          tone={
            props.settings.tossConnection === "connected"
              ? "primary"
              : "neutral"
          }
        >
          {settingsText(
            props.settings.tossConnection === "connected"
              ? "tossConnected"
              : "tossNeedsKey",
            "ko",
          )}
        </Badge>
      </Inline>
      <Text variant="caption" tone="ink-muted">
        {settingsText("tossPasteHint", "ko")}
      </Text>
    </Stack>
  </Card>
);

const InvestorProfileCard = (props: {
  readonly settings: SettingsView;
}): ReactElement => (
  <Card title={settingsText("profileTitle", "ko")}>
    <Stack gap="space-3">
      <Inline gap="space-2" align="center" wrap="wrap">
        <Text variant="label">{settingsText("horizonLabel", "ko")}</Text>
        <Badge tone="neutral">
          {settingsText(horizonKeys[props.settings.horizon], "ko")}
        </Badge>
        <Text variant="label">{settingsText("riskLabel", "ko")}</Text>
        <Badge tone="neutral">
          {settingsText(riskKeys[props.settings.riskAppetite], "ko")}
        </Badge>
      </Inline>
      <Inline gap="space-3" align="center" wrap="wrap">
        <Text variant="label">{settingsText("instrumentCap", "ko")}</Text>
        <Percent value={props.settings.instrumentCap} />
        <Text variant="label">{settingsText("instrumentWarning", "ko")}</Text>
        <Percent value={props.settings.instrumentWarning} />
        <Text variant="label">{settingsText("sectorCap", "ko")}</Text>
        <Percent value={props.settings.sectorCap} />
      </Inline>
      <Inline gap="space-3" align="center" wrap="wrap">
        <Text variant="label">{settingsText("exclusionsSectors", "ko")}</Text>
        <DataText
          value={props.settings.excludedSectorCount}
          variant="body-sm"
        />
        <Text variant="label">{settingsText("exclusionsTypes", "ko")}</Text>
        <DataText
          value={props.settings.excludedSecurityTypeCount}
          variant="body-sm"
        />
      </Inline>
    </Stack>
  </Card>
);

const OrderSwitchCard = (props: {
  readonly settings: SettingsView;
  readonly feedback: OrderSwitchFeedback;
  readonly onToggle: (next: OrderPermissionState) => void;
}): ReactElement => {
  const noteKey = feedbackKey(props.feedback);
  const switchControl = (
    <Switch
      label={settingsText("orderSwitchTitle", "ko")}
      description={settingsText("orderSwitchDescription", "ko")}
      checked={props.settings.orderEnabled}
      disabled={props.feedback === "submitting" ? "disabled" : "enabled"}
      onChange={props.onToggle}
    />
  );
  return (
    <Card title={settingsText("orderSwitchTitle", "ko")}>
      {noteKey === undefined ? (
        switchControl
      ) : (
        <Stack gap="space-2">
          {switchControl}
          <Text
            variant="caption"
            tone={props.feedback === "changed" ? "primary" : "alert"}
          >
            {settingsText(noteKey, "ko")}
          </Text>
        </Stack>
      )}
    </Card>
  );
};

const SettingsBody = (props: {
  readonly settings: SettingsView;
  readonly onOpenStatus: () => void;
}): ReactElement => {
  const orderSwitch = useOrderPermissionSwitch();
  return (
    <Stack gap="space-6">
      <Section title={settingsText("basicSection", "ko")}>
        <Stack gap="space-4">
          <TossConnectionCard settings={props.settings} />
          <InvestorProfileCard settings={props.settings} />
          <OrderSwitchCard
            settings={props.settings}
            feedback={orderSwitch.feedback}
            onToggle={orderSwitch.toggle}
          />
        </Stack>
      </Section>
      <AdvancedSettings
        settings={props.settings}
        onOpenStatus={props.onOpenStatus}
      />
    </Stack>
  );
};

const SettingsContent = (props: {
  readonly view: SettingsQueryState;
  readonly retry: () => void;
  readonly onOpenStatus: () => void;
}): ReactElement => {
  switch (props.view.status) {
    case "loading":
      return (
        <Card title={settingsText("loadingTitle", "ko")}>
          <Skeleton variant="text" />
        </Card>
      );
    case "error":
      return (
        <EmptyState
          title={settingsText("errorTitle", "ko")}
          action={{
            label: settingsText("retryAction", "ko"),
            onPress: props.retry,
          }}
        />
      );
    case "ready":
      return (
        <SettingsBody
          settings={props.view.data}
          onOpenStatus={props.onOpenStatus}
        />
      );
  }
};

export const SettingsScreen = (props: Props): ReactElement => {
  const query = useSettingsQuery();
  return (
    <Page
      title={settingsText("title", "ko")}
      actions={{
        label: settingsText("backToHome", "ko"),
        onPress: props.onBack,
      }}
    >
      <Stack gap="space-6">
        {[
          <SettingsContent
            key="settings"
            view={query.view}
            retry={query.retry}
            onOpenStatus={props.onOpenStatus}
          />,
          ...(props.system === null ? [] : [props.system]),
        ]}
      </Stack>
    </Page>
  );
};
