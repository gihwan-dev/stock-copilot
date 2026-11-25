import { useState, type ReactElement } from "react";
import {
  Badge,
  DataText,
  Inline,
  ListRow,
  Percent,
  Stack,
  Text,
} from "../../../shared/ds";
import type {
  HorizonOption,
  OrderPermissionState,
  RiskAppetiteOption,
  SettingsView,
} from "../../../shared/ipc";
import { settingsText, type SettingsMessageKey } from "../messages";
import type { OrderSwitchFeedback } from "../model/settings";

export type TossEntry =
  | { readonly kind: "absent" }
  | { readonly kind: "present"; readonly open: () => void };

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

export const TossConnectionRow = (props: {
  readonly settings: SettingsView;
  readonly entry: TossEntry;
}): ReactElement => (
  <ListRow
    title={settingsText("tossTitle", "ko")}
    value={settingsText(
      props.settings.tossConnection === "connected"
        ? "tossConnected"
        : "tossNeedsKey",
      "ko",
    )}
    trailing={
      props.entry.kind === "present"
        ? {
            kind: "link",
            label: settingsText("tossPasteAction", "ko"),
            onPress: props.entry.open,
          }
        : { kind: "none" }
    }
  />
);

const ProfileDetails = (props: {
  readonly settings: SettingsView;
}): ReactElement => (
  <Stack gap="space-3">
    <Inline gap="space-2" align="center" wrap="wrap">
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
      <DataText value={props.settings.excludedSectorCount} variant="body-sm" />
      <Text variant="label">{settingsText("exclusionsTypes", "ko")}</Text>
      <DataText
        value={props.settings.excludedSecurityTypeCount}
        variant="body-sm"
      />
    </Inline>
  </Stack>
);

export const InvestorProfileRow = (props: {
  readonly settings: SettingsView;
}): ReactElement => {
  const [fold, setFold] = useState<"open" | "closed">("closed");
  const toggle = (): void => {
    setFold(fold === "open" ? "closed" : "open");
  };
  const trailing = {
    kind: "fold",
    label: settingsText("profileDetails", "ko"),
    state: fold,
    onPress: toggle,
  } as const;
  const value = settingsText(horizonKeys[props.settings.horizon], "ko");
  return fold === "open" ? (
    <ListRow
      title={settingsText("profileTitle", "ko")}
      value={value}
      trailing={trailing}
    >
      <ProfileDetails settings={props.settings} />
    </ListRow>
  ) : (
    <ListRow
      title={settingsText("profileTitle", "ko")}
      value={value}
      trailing={trailing}
    />
  );
};

export const OrderSwitchRow = (props: {
  readonly settings: SettingsView;
  readonly feedback: OrderSwitchFeedback;
  readonly onToggle: (next: OrderPermissionState) => void;
}): ReactElement => {
  const noteKey = feedbackKey(props.feedback);
  const trailing = {
    kind: "switch",
    checked: props.settings.orderEnabled,
    disabled: props.feedback === "submitting" ? "disabled" : "enabled",
    onChange: props.onToggle,
  } as const;
  return noteKey === undefined ? (
    <ListRow
      title={settingsText("orderSwitchTitle", "ko")}
      value={settingsText("orderSwitchDescription", "ko")}
      trailing={trailing}
    />
  ) : (
    <ListRow
      title={settingsText("orderSwitchTitle", "ko")}
      value={settingsText("orderSwitchDescription", "ko")}
      trailing={trailing}
    >
      <Text
        variant="caption"
        tone={props.feedback === "changed" ? "primary" : "alert"}
      >
        {settingsText(noteKey, "ko")}
      </Text>
    </ListRow>
  );
};
