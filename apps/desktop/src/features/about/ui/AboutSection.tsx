import type { ReactElement } from "react";
import { Section, Stack, Text } from "../../../shared/ds";
import { text } from "../../../shared/lib";
import { aboutText } from "../messages";

export const AboutSection = (): ReactElement => (
  <Section title={aboutText("title", "ko")}>
    <Stack gap="space-2">
      <Text variant="label">{text("app.title", "ko")}</Text>
      <Text variant="body-sm" tone="ink-muted">
        {text("notice.appInfo", "ko")}
      </Text>
    </Stack>
  </Section>
);
