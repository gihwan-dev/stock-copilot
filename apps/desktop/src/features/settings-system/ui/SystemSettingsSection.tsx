import type { ReactElement } from "react";
import { Card, Section, Stack, Text } from "../../../shared/ds";
import { systemText } from "../messages";
import { AccountCard } from "./AccountCard";
import { DataCard } from "./DataCard";

const SyncCard = (): ReactElement => (
  <Card title={systemText("system.syncTitle", "ko")}>
    <Stack gap="space-2">
      <Text>{systemText("system.syncHoldings", "ko")}</Text>
      <Text>{systemText("system.syncTransactions", "ko")}</Text>
      <Text tone="ink-muted">{systemText("system.syncSession", "ko")}</Text>
    </Stack>
  </Card>
);

export const SystemSettingsSection = (): ReactElement => (
  <Section title={systemText("system.title", "ko")}>
    <AccountCard />
    <SyncCard />
    <DataCard />
  </Section>
);
