import type { ReactElement } from "react";
import {
  Card,
  EmptyState,
  Page,
  Section,
  Skeleton,
  Stack,
} from "../../../shared/ds";
import type { SettingsView } from "../../../shared/ipc";
import { settingsText } from "../messages";
import {
  useOrderPermissionSwitch,
  useSettingsQuery,
  type SettingsQueryState,
} from "../model/settings";
import { AdvancedSettings } from "./AdvancedSettings";
import {
  InvestorProfileRow,
  OrderSwitchRow,
  TossConnectionRow,
  type TossEntry,
} from "./BasicRows";

export type { TossEntry } from "./BasicRows";

type Props = {
  readonly tossEntry: TossEntry;
  readonly onBack: () => void;
  readonly onOpenStatus: () => void;
  readonly system: ReactElement | null;
};

const SettingsBody = (props: {
  readonly settings: SettingsView;
  readonly entry: TossEntry;
  readonly onOpenStatus: () => void;
}): ReactElement => {
  const orderSwitch = useOrderPermissionSwitch();
  return (
    <Stack gap="space-6">
      <Section title={settingsText("basicSection", "ko")}>
        <Card>
          <TossConnectionRow settings={props.settings} entry={props.entry} />
          <InvestorProfileRow settings={props.settings} />
          <OrderSwitchRow
            settings={props.settings}
            feedback={orderSwitch.feedback}
            onToggle={orderSwitch.toggle}
          />
        </Card>
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
  readonly entry: TossEntry;
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
          entry={props.entry}
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
            entry={props.tossEntry}
            retry={query.retry}
            onOpenStatus={props.onOpenStatus}
          />,
          ...(props.system === null ? [] : [props.system]),
        ]}
      </Stack>
    </Page>
  );
};
