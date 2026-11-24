import { createContext, use, type ReactElement } from "react";
import {
  unavailableSettingsClient,
  type SettingsClient,
} from "../../../shared/ipc";

const ClientContext = createContext<SettingsClient>(unavailableSettingsClient);

export const OnboardingClientProvider = (props: {
  readonly client: SettingsClient;
  readonly children: ReactElement;
}): ReactElement => (
  <ClientContext value={props.client}>{props.children}</ClientContext>
);

export const useOnboardingClient = (): SettingsClient => use(ClientContext);
