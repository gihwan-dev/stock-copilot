import type { ReactElement } from "react";
import {
  Card,
  Section,
  SegmentedControl,
  Stack,
  Text,
} from "../../../shared/ds";
import {
  emptyInputValue,
  parseInputValue,
  type InputValue,
} from "../../../shared/lib";
import { useAppearanceSettings } from "../model/appearance";
import { settingsText } from "../messages";

type Pixels = "0" | "4" | "8" | "12" | "16";

const inputValue = (value: Pixels): InputValue => {
  const parsed = parseInputValue(value);
  return parsed.kind === "ok" ? parsed.value : emptyInputValue;
};

const common = [
  { value: inputValue("0"), label: settingsText("pixels0", "ko") },
  { value: inputValue("4"), label: settingsText("pixels4", "ko") },
  { value: inputValue("8"), label: settingsText("pixels8", "ko") },
] as const;

const gapOptions = [
  ...common,
  { value: inputValue("12"), label: settingsText("pixels12", "ko") },
] as const;

const paddingOptions = [
  ...common,
  { value: inputValue("16"), label: settingsText("pixels16", "ko") },
] as const;

const rows = [
  { field: "gap", label: settingsText("widgetGap", "ko"), options: gapOptions },
  {
    field: "padding",
    label: settingsText("widgetPadding", "ko"),
    options: paddingOptions,
  },
  {
    field: "radius",
    label: settingsText("widgetRadius", "ko"),
    options: gapOptions,
  },
] as const;

export const AppearanceSettings = (): ReactElement => {
  const { prefs, change } = useAppearanceSettings();
  return (
    <Section title={settingsText("appearanceTitle", "ko")}>
      <Card>
        {rows.map(({ field, label, options }): ReactElement => (
          <Stack key={field} gap="space-1">
            <Text variant="label">{label}</Text>
            <SegmentedControl
              label={label}
              options={options}
              value={inputValue(prefs[field])}
              onChange={(value): void => {
                change(field, value);
              }}
            />
          </Stack>
        ))}
      </Card>
    </Section>
  );
};
