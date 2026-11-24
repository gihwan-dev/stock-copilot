import type { ReactElement } from "react";
import {
  SystemSettingsClientProvider,
  SystemSettingsSection,
} from "../features/settings-system";
import type { SettingsClient } from "../shared/ipc";

export const SettingsClients = (props: {
  readonly client: SettingsClient;
  readonly children: ReactElement;
}): ReactElement => (
  <SystemSettingsClientProvider client={props.client}>
    {props.children}
  </SystemSettingsClientProvider>
);

export const SettingsSections = (): ReactElement => <SystemSettingsSection />;
