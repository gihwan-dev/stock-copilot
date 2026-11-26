import type { ReactElement } from "react";
import {
  Card,
  DataText,
  IconButton,
  Inline,
  Stack,
  Text,
  TimestampText,
} from "../../../shared/ds";
import type { InstrumentNewsLine } from "../../../shared/ipc";
import type { ExternalUrl } from "../../../shared/lib";
import { instrumentsText } from "../messages";
import { useOpenArticle } from "../model/instruments";

const Headline = (props: {
  readonly line: InstrumentNewsLine;
}): ReactElement => (
  <Stack gap="space-1">
    <DataText value={props.line.title} variant="body-sm" />
    <Inline gap="space-2" align="center" wrap="wrap">
      <DataText value={props.line.source} variant="caption" />
      <TimestampText value={props.line.publishedAt} variant="caption" />
    </Inline>
  </Stack>
);

const NewsRow = (props: {
  readonly line: InstrumentNewsLine;
  readonly onOpen: (url: ExternalUrl) => void;
}): ReactElement => {
  const url = props.line.url;
  if (url === undefined) return <Headline line={props.line} />;
  return (
    <Inline gap="space-2" align="start" justify="between">
      <Headline line={props.line} />
      <IconButton
        icon="external-link"
        label={instrumentsText("newsOpen", "ko")}
        onClick={(): void => {
          props.onOpen(url);
        }}
      />
    </Inline>
  );
};

export const RecentNews = (props: {
  readonly news: readonly InstrumentNewsLine[];
}): ReactElement => {
  const open = useOpenArticle();
  return (
    <Card title={instrumentsText("newsTitle", "ko")}>
      {props.news.length === 0 ? (
        <Text variant="body-sm" tone="ink-muted">
          {instrumentsText("newsEmpty", "ko")}
        </Text>
      ) : (
        <Stack gap="space-3">
          {props.news.map((line): ReactElement => (
            <NewsRow key={line.id} line={line} onOpen={open} />
          ))}
        </Stack>
      )}
    </Card>
  );
};
