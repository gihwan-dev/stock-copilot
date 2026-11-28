import type { ReactElement } from "react";
import {
  Card,
  EmptyState,
  Page,
  Skeleton,
  Stack,
  Text,
} from "../../../shared/ds";
import type { BriefingSummary, ScreenSources } from "../../../shared/ipc";
import { briefingText } from "../messages";
import { useBriefings, useOpenSource } from "../model/briefings";
import { Body } from "./BriefingReader";
import { BriefingView, type BriefingLayout } from "./BriefingView";

type Load = ScreenSources["briefings"];

const Loading = (props: { readonly layout?: BriefingLayout }): ReactElement => (
  <Card
    title={briefingText("loadingTitle", "ko")}
    layout={props.layout ?? "flow"}
  >
    <Skeleton variant="text" lines={3} />
  </Card>
);

const Failed = (props: {
  readonly retry: () => void;
  readonly layout?: BriefingLayout;
}): ReactElement => (
  <Card
    title={briefingText("errorTitle", "ko")}
    layout={props.layout ?? "flow"}
    actions={{ label: briefingText("retry", "ko"), onPress: props.retry }}
  >
    <Stack gap="space-1">{[]}</Stack>
  </Card>
);

const Empty = (props: { readonly layout: BriefingLayout }): ReactElement => (
  <Card title={briefingText("emptyTitle", "ko")} layout={props.layout}>
    <Text variant="body-sm" tone="ink-muted">
      {briefingText("emptyBody", "ko")}
    </Text>
  </Card>
);

const Summary = (props: {
  readonly latest: BriefingSummary;
  readonly layout: BriefingLayout;
  readonly openSource: ReturnType<typeof useOpenSource>;
  readonly open: () => void;
}): ReactElement => (
  <Card
    title={briefingText(props.latest.kind, "ko")}
    layout={props.layout}
    actions={{
      label: briefingText("openBriefing", "ko"),
      onPress: props.open,
    }}
  >
    {props.latest.outcome.kind === "completed" ? (
      <Body
        briefing={props.latest}
        onOpenSource={props.openSource}
        presentation="dashboard"
      />
    ) : (
      <Text>{briefingText(props.latest.outcome.reason, "ko")}</Text>
    )}
  </Card>
);

const otherSessions = (
  latest: BriefingSummary,
  past: readonly BriefingSummary[],
): readonly BriefingSummary[] => {
  return (["morning", "pre-us-market"] as const)
    .filter((kind) => kind !== latest.kind)
    .map((kind) => past.find((briefing) => briefing.kind === kind))
    .filter((briefing) => briefing !== undefined);
};

const SessionCards = ({
  latest,
  past,
  layout,
  openSource,
  openHistory,
}: {
  readonly latest: BriefingSummary;
  readonly past: readonly BriefingSummary[];
  readonly layout: BriefingLayout;
  readonly openSource: ReturnType<typeof useOpenSource>;
  readonly openHistory: () => void;
}): ReactElement => {
  return (
    <Stack gap="space-4">
      <BriefingView
        layout={layout}
        briefing={latest}
        onOpenSource={openSource}
        onOpenHistory={past.length === 0 ? undefined : openHistory}
      />
      {otherSessions(latest, past).map((briefing) => (
        <BriefingView
          key={briefing.id}
          briefing={briefing}
          onOpenSource={openSource}
          onOpenHistory={undefined}
        />
      ))}
    </Stack>
  );
};

export const BriefingCard = (props: {
  readonly load: Load;
  readonly presentation?: "summary" | "full";
  readonly onOpenHistory: () => void;
  readonly layout?: BriefingLayout;
}): ReactElement => {
  const briefings = useBriefings(props.load, "latest");
  const openSource = useOpenSource();
  const layout = props.layout ?? "flow";
  switch (briefings.view.status) {
    case "loading":
      return <Loading layout={layout} />;
    case "empty":
      return <Empty layout={layout} />;
    case "error":
      return <Failed retry={briefings.retry} layout={layout} />;
    case "ready": {
      const latest = briefings.view.data.latest;
      if (latest === undefined) return <Empty layout={layout} />;
      if (props.presentation === "summary")
        return (
          <Summary
            latest={latest}
            layout={layout}
            openSource={openSource}
            open={props.onOpenHistory}
          />
        );
      return (
        <SessionCards
          latest={latest}
          past={briefings.view.data.past}
          layout={layout}
          openSource={openSource}
          openHistory={props.onOpenHistory}
        />
      );
    }
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
