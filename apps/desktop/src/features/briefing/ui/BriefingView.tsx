import type { ReactElement } from "react";
import {
  Badge,
  Card,
  DataText,
  Inline,
  Markdown,
  Stack,
  Text,
  TimestampText,
} from "../../../shared/ds";
import type {
  BriefingSourceKind,
  BriefingSourceView,
  BriefingSummary,
} from "../../../shared/ipc";
import type { ExternalUrl } from "../../../shared/lib";
import { briefingText, type BriefingMessageKey } from "../messages";
import { linkVerdict } from "../model/briefings";

const sourceKeys = {
  web: "sourceWeb",
  news: "sourceNews",
  disclosure: "sourceDisclosure",
} as const satisfies Record<BriefingSourceKind, BriefingMessageKey>;

const SourceRow = (props: {
  readonly source: BriefingSourceView;
}): ReactElement => (
  <Inline gap="space-2" align="center" wrap="wrap">
    <Badge tone={props.source.kind === "web" ? "alert" : "neutral"}>
      {briefingText(sourceKeys[props.source.kind], "ko")}
    </Badge>
    <DataText value={props.source.title} variant="body-sm" />
  </Inline>
);

const Sources = (props: {
  readonly sources: readonly BriefingSourceView[];
}): ReactElement =>
  props.sources.length === 0 ? (
    <Badge tone="alert">{briefingText("noSources", "ko")}</Badge>
  ) : (
    <Stack gap="space-1">
      <Text variant="label">{briefingText("sources", "ko")}</Text>
      {props.sources.map((source): ReactElement => (
        <SourceRow key={source.id} source={source} />
      ))}
    </Stack>
  );

const Body = (props: {
  readonly briefing: BriefingSummary;
  readonly onOpenSource: (url: ExternalUrl) => void;
}): ReactElement => {
  const outcome = props.briefing.outcome;
  switch (outcome.kind) {
    case "completed":
      return (
        <Stack gap="space-3">
          <Markdown
            content={outcome.body}
            linkAllowed={(url) => linkVerdict(props.briefing, url)}
            onOpenLink={props.onOpenSource}
          />
          <Sources sources={outcome.sources} />
          <Text variant="caption" tone="ink-subtle">
            {briefingText("aiNotice", "ko")}
          </Text>
        </Stack>
      );
    case "failed":
      return (
        <Stack gap="space-1">
          <Text variant="body-sm">{briefingText("failedTitle", "ko")}</Text>
          <Text variant="caption" tone="ink-muted">
            {briefingText(outcome.reason, "ko")}
          </Text>
        </Stack>
      );
  }
};

const Content = (props: {
  readonly briefing: BriefingSummary;
  readonly onOpenSource: (url: ExternalUrl) => void;
}): ReactElement => (
  <Stack gap="space-3">
    <Inline gap="space-2" align="center" wrap="wrap">
      <Text variant="caption" tone="ink-muted">
        {briefingText("finishedAt", "ko")}
      </Text>
      <TimestampText value={props.briefing.finishedAt} />
    </Inline>
    <Body briefing={props.briefing} onOpenSource={props.onOpenSource} />
  </Stack>
);

export type BriefingLayout = "flow" | "fill";

export const BriefingView = (props: {
  readonly briefing: BriefingSummary;
  readonly onOpenSource: (url: ExternalUrl) => void;
  readonly onOpenHistory: (() => void) | undefined;
  readonly layout?: BriefingLayout;
}): ReactElement =>
  props.onOpenHistory === undefined ? (
    <Card
      title={briefingText(props.briefing.kind, "ko")}
      layout={props.layout ?? "flow"}
    >
      <Content briefing={props.briefing} onOpenSource={props.onOpenSource} />
    </Card>
  ) : (
    <Card
      title={briefingText(props.briefing.kind, "ko")}
      layout={props.layout ?? "flow"}
      actions={{
        label: briefingText("openHistory", "ko"),
        onPress: props.onOpenHistory,
      }}
    >
      <Content briefing={props.briefing} onOpenSource={props.onOpenSource} />
    </Card>
  );
