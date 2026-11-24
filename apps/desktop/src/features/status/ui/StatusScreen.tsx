import type { ReactElement } from "react";
import {
  Card,
  DataText,
  EmptyState,
  Inline,
  Page,
  Section,
  Skeleton,
  Stack,
  Text,
  TimestampText,
} from "../../../shared/ds";
import type { ScreenSources, StatusView } from "../../../shared/ipc";
import { statusText } from "../messages";
import { useStatus } from "../model/status";
import { RecordsCard, SourcesCard, UsageCard } from "./StatusTables";

const RuntimeCard = (props: { readonly view: StatusView }): ReactElement => (
  <Card title={statusText("runtimeTitle", "ko")}>
    {props.view.aiRuntime.kind === "ready" ? (
      <Text variant="body-sm">{statusText("runtimeReady", "ko")}</Text>
    ) : (
      <Inline gap="space-2" align="center" wrap="wrap">
        <Text variant="body-sm">{statusText("runtimeUnavailable", "ko")}</Text>
        <DataText value={props.view.aiRuntime.reason} variant="body-sm" />
      </Inline>
    )}
  </Card>
);

const LimitCard = (props: { readonly view: StatusView }): ReactElement => (
  <Card title={statusText("limitTitle", "ko")}>
    {props.view.aiLimit.kind === "available" ? (
      <Text variant="body-sm">{statusText("limitAvailable", "ko")}</Text>
    ) : (
      <Inline gap="space-2" align="center" wrap="wrap">
        <Text variant="body-sm">{statusText("limitWaiting", "ko")}</Text>
        <TimestampText value={props.view.aiLimit.resumesAt} variant="body-sm" />
      </Inline>
    )}
  </Card>
);

const StatusReady = (props: { readonly view: StatusView }): ReactElement => (
  <Stack gap="space-6">
    <Text variant="body-sm" tone="ink-muted">
      {statusText("intro", "ko")}
    </Text>
    <Section title={statusText("aiSection", "ko")}>
      <Stack gap="space-4">
        <RuntimeCard view={props.view} />
        <LimitCard view={props.view} />
        <UsageCard usage={props.view.aiUsage} />
      </Stack>
    </Section>
    <Section title={statusText("sourcesSection", "ko")}>
      <SourcesCard sources={props.view.sources} />
    </Section>
    <Section title={statusText("recordsSection", "ko")}>
      <RecordsCard records={props.view.records} />
    </Section>
  </Stack>
);

const StatusBody = (props: {
  readonly load: ScreenSources["status"];
}): ReactElement => {
  const status = useStatus(props.load);
  switch (status.view.status) {
    case "loading":
      return (
        <Card title={statusText("loadingTitle", "ko")}>
          <Skeleton variant="text" lines={3} />
        </Card>
      );
    case "empty":
      return (
        <EmptyState title={statusText("emptyTitle", "ko")}>
          {statusText("emptyBody", "ko")}
        </EmptyState>
      );
    case "error":
      return (
        <EmptyState
          title={statusText("errorTitle", "ko")}
          action={{ label: statusText("retry", "ko"), onPress: status.retry }}
        />
      );
    case "ready":
      return <StatusReady view={status.view.data} />;
  }
};

export const StatusScreen = (props: {
  readonly load: ScreenSources["status"];
  readonly onBack: () => void;
}): ReactElement => (
  <Page
    title={statusText("title", "ko")}
    actions={{
      label: statusText("backToSettings", "ko"),
      onPress: props.onBack,
    }}
  >
    <StatusBody load={props.load} />
  </Page>
);
