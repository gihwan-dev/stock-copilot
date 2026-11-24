import { createContext, use, type ReactElement } from "react";
import {
  unavailableSettingsClient,
  type SettingsClient,
} from "../../../shared/ipc";

const ClientContext = createContext<SettingsClient>(unavailableSettingsClient);

export const SystemSettingsClientProvider = (props: {
  readonly client: SettingsClient;
  readonly children: ReactElement;
}): ReactElement => (
  <ClientContext value={props.client}>{props.children}</ClientContext>
);

export const useSystemClient = (): SettingsClient => use(ClientContext);
