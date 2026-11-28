import { createContext, use, type ReactElement } from "react";
import type { useWidgetPreferences } from "../../../shared/platform";

type Appearance = ReturnType<typeof useWidgetPreferences>;

const AppearanceSettingsContext = createContext<Appearance>({
  prefs: { gap: "0", padding: "8", radius: "0" },
  change: (): void => undefined,
});

export const AppearanceSettingsProvider = (props: {
  readonly appearance: Appearance;
  readonly children: ReactElement;
}): ReactElement => (
  <AppearanceSettingsContext value={props.appearance}>
    {props.children}
  </AppearanceSettingsContext>
);

export const useAppearanceSettings = (): Appearance =>
  use(AppearanceSettingsContext);
