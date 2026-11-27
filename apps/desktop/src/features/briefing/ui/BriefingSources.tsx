import type { ReactElement } from "react";
import {
  Badge,
  Button,
  DataText,
  Inline,
  Stack,
  Text,
  TimestampText,
} from "../../../shared/ds";
import type {
  BriefingSourceKind,
  BriefingSourceView,
} from "../../../shared/ipc";
import type { ExternalUrl } from "../../../shared/lib";
import { briefingText, type BriefingMessageKey } from "../messages";

const sourceKeys = {
  web: "sourceWeb",
  news: "sourceNews",
  disclosure: "sourceDisclosure",
} as const satisfies Record<BriefingSourceKind, BriefingMessageKey>;

const SourceTitle = (props: {
  readonly source: BriefingSourceView;
  readonly onOpenSource: (url: ExternalUrl) => void;
}): ReactElement => {
  const url = props.source.url;
  return (
    <Inline gap="space-1" align="center" wrap="wrap">
      {[
        <DataText key="title" value={props.source.title} variant="body-sm" />,
        ...(url === undefined
          ? [
              <Text key="no-original" variant="caption" tone="ink-subtle">
                {briefingText("noOriginal", "ko")}
              </Text>,
            ]
          : [
              <Button
                key="open"
                variant="ghost"
                size="control-height-sm"
                onClick={(): void => {
                  props.onOpenSource(url);
                }}
              >
                {briefingText("openSource", "ko")}
              </Button>,
            ]),
      ]}
    </Inline>
  );
};

const SourceRow = (props: {
  readonly source: BriefingSourceView;
  readonly onOpenSource: (url: ExternalUrl) => void;
}): ReactElement => (
  <Inline gap="space-2" align="center" wrap="wrap">
    {[
      ...(props.source.cite === undefined
        ? []
        : [<DataText key="cite" value={props.source.cite} variant="code" />]),
      <Badge
        key="kind"
        tone={props.source.kind === "web" ? "alert" : "neutral"}
      >
        {briefingText(sourceKeys[props.source.kind], "ko")}
      </Badge>,
      ...(props.source.publisher === undefined
        ? []
        : [
            <DataText
              key="publisher"
              value={props.source.publisher}
              variant="caption"
            />,
          ]),
      <SourceTitle
        key="title"
        source={props.source}
        onOpenSource={props.onOpenSource}
      />,
      ...(props.source.observedAt === undefined
        ? []
        : [
            <Text key="timeLabel" variant="caption">
              {briefingText("observedAt", "ko")}
            </Text>,
            <TimestampText
              key="time"
              value={props.source.observedAt}
              variant="caption"
            />,
          ]),
    ]}
  </Inline>
);

export const Sources = (props: {
  readonly sources: readonly BriefingSourceView[];
  readonly onOpenSource: (url: ExternalUrl) => void;
}): ReactElement =>
  props.sources.length === 0 ? (
    <Badge tone="alert">{briefingText("noSources", "ko")}</Badge>
  ) : (
    <Stack gap="space-1">
      <Text variant="label">{briefingText("sources", "ko")}</Text>
      {props.sources.map((source): ReactElement => (
        <SourceRow
          key={source.id}
          source={source}
          onOpenSource={props.onOpenSource}
        />
      ))}
    </Stack>
  );
