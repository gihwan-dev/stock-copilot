import type { ReactElement } from "react";
import {
  AiSettingsClientProvider,
  AiSettingsSection,
} from "../features/settings-ai";
import {
  NewsSettingsClientProvider,
  NewsSettingsSection,
} from "../features/settings-news";
import {
  PrinciplesSettingsClientProvider,
  PrinciplesSettingsSection,
} from "../features/settings-principles";
import {
  SystemSettingsClientProvider,
  SystemSettingsSection,
} from "../features/settings-system";
import { Stack } from "../shared/ds";
import type { SettingsClient } from "../shared/ipc";

export const SettingsClients = (props: {
  readonly client: SettingsClient;
  readonly children: ReactElement;
}): ReactElement => (
  <SystemSettingsClientProvider client={props.client}>
    <AiSettingsClientProvider client={props.client}>
      <NewsSettingsClientProvider client={props.client}>
        <PrinciplesSettingsClientProvider client={props.client}>
          {props.children}
        </PrinciplesSettingsClientProvider>
      </NewsSettingsClientProvider>
    </AiSettingsClientProvider>
  </SystemSettingsClientProvider>
);

export const SettingsSections = (): ReactElement => (
  <Stack gap="space-8">
    <AiSettingsSection />
    <NewsSettingsSection />
    <PrinciplesSettingsSection />
    <SystemSettingsSection />
  </Stack>
);
