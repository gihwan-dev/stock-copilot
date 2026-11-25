import type { ReactElement } from "react";
import {
  Card,
  EmptyState,
  Page,
  Skeleton,
  Stack,
  Text,
} from "../../../shared/ds";
import type { ScreenSources } from "../../../shared/ipc";
import { briefingText } from "../messages";
import { useBriefings, useOpenSource } from "../model/briefings";
import { BriefingView } from "./BriefingView";

type Load = ScreenSources["briefings"];

const Loading = (): ReactElement => (
  <Card title={briefingText("loadingTitle", "ko")}>
    <Skeleton variant="text" lines={3} />
  </Card>
);

const Failed = (props: { readonly retry: () => void }): ReactElement => (
  <Card
    title={briefingText("errorTitle", "ko")}
    actions={{ label: briefingText("retry", "ko"), onPress: props.retry }}
  >
    <Stack gap="space-1">{[]}</Stack>
  </Card>
);

export const BriefingCard = (props: {
  readonly load: Load;
  readonly onOpenHistory: () => void;
}): ReactElement => {
  const briefings = useBriefings(props.load, "latest");
  const openSource = useOpenSource();
  switch (briefings.view.status) {
    case "loading":
      return <Loading />;
    case "empty":
      return (
        <Card title={briefingText("emptyTitle", "ko")}>
          <Text variant="body-sm" tone="ink-muted">
            {briefingText("emptyBody", "ko")}
          </Text>
        </Card>
      );
    case "error":
      return <Failed retry={briefings.retry} />;
    case "ready":
      return briefings.view.data.latest === undefined ? (
        <Card title={briefingText("emptyTitle", "ko")}>
          <Text variant="body-sm" tone="ink-muted">
            {briefingText("emptyBody", "ko")}
          </Text>
        </Card>
      ) : (
        <BriefingView
          briefing={briefings.view.data.latest}
          onOpenSource={openSource}
          onOpenHistory={
            briefings.view.data.past.length === 0
              ? undefined
              : props.onOpenHistory
          }
        />
      );
  }
};

const HistoryBody = (props: { readonly load: Load }): ReactElement => {
  const briefings = useBriefings(props.load, "past");
  const openSource = useOpenSource();
  switch (briefings.view.status) {
    case "loading":
      return <Loading />;
    case "empty":
      return (
        <EmptyState title={briefingText("historyEmptyTitle", "ko")}>
          {briefingText("historyEmptyBody", "ko")}
        </EmptyState>
      );
    case "error":
      return <Failed retry={briefings.retry} />;
    case "ready":
      return (
        <Stack gap="space-4">
          {briefings.view.data.past.map((briefing): ReactElement => (
            <BriefingView
              key={briefing.id}
              briefing={briefing}
              onOpenSource={openSource}
              onOpenHistory={undefined}
            />
          ))}
        </Stack>
      );
  }
};

export const BriefingHistoryScreen = (props: {
  readonly load: Load;
  readonly onBack: () => void;
}): ReactElement => (
  <Page
    title={briefingText("historyTitle", "ko")}
    actions={{
      label: briefingText("backToHome", "ko"),
      onPress: props.onBack,
    }}
  >
    <HistoryBody load={props.load} />
  </Page>
);
