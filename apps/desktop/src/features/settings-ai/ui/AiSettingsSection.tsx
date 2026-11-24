import type { ReactElement } from "react";
import { Card, Section, Skeleton, Stack, Text } from "../../../shared/ds";
import type { UiText } from "../../../shared/lib";
import { aiText } from "../messages";
import {
  useAiSaver,
  useAiSettings,
  useJobSaver,
  useScheduledJobs,
  type SaveState,
} from "../model";
import { BudgetCard } from "./BudgetCard";
import { useAiClient } from "./client";
import { ConfigCard } from "./ConfigCard";
import { JobsCard } from "./JobsCard";
import { ModelsCard } from "./ModelsCard";
import { PrivacyCard } from "./PrivacyCard";

const saveText = (state: SaveState): UiText | undefined => {
  switch (state) {
    case "idle":
    case "pending":
      return undefined;
    case "saved":
      return aiText("ai.saved", "ko");
    case "failed":
      return aiText("ai.saveFailed", "ko");
  }
};

const Notice = (props: { readonly state: SaveState }): ReactElement | null => {
  const sentence = saveText(props.state);
  if (sentence === undefined) return null;
  return (
    <Text tone={props.state === "failed" ? "alert" : "ink-muted"}>
      {sentence}
    </Text>
  );
};

const Loading = (): ReactElement => (
  <Card title={aiText("ai.loading", "ko")}>
    <Skeleton variant="text" />
  </Card>
);

const Failed = (): ReactElement => (
  <Text tone="alert">{aiText("ai.loadFailed", "ko")}</Text>
);

const AiCards = (): ReactElement => {
  const client = useAiClient();
  const view = useAiSettings(client);
  const saver = useAiSaver(client);
  switch (view.status) {
    case "loading":
      return <Loading />;
    case "error":
      return <Failed />;
    case "ready":
      return (
        <Stack gap="space-6">
          <Notice state={saver.state} />
          <ModelsCard settings={view.data} save={saver.save} />
          <BudgetCard settings={view.data} save={saver.save} />
          <ConfigCard settings={view.data} save={saver.save} />
        </Stack>
      );
  }
};

const JobCards = (): ReactElement => {
  const client = useAiClient();
  const view = useScheduledJobs(client);
  const saver = useJobSaver(client);
  switch (view.status) {
    case "loading":
      return <Loading />;
    case "error":
      return <Failed />;
    case "ready":
      return (
        <Stack gap="space-6">
          <Notice state={saver.state} />
          <JobsCard jobs={view.data} save={saver.save} />
        </Stack>
      );
  }
};

export const AiSettingsSection = (): ReactElement => (
  <Section title={aiText("ai.title", "ko")}>
    <AiCards />
    <JobCards />
    <PrivacyCard />
  </Section>
);
