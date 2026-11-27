import type { ComponentProps, ReactElement } from "react";
import {
  Dialog,
  Inline,
  Markdown,
  Stack,
  Text,
  TimestampText,
} from "../../../shared/ds";
import { briefingCitations } from "../model/citations";
import type { BriefingSummary } from "../../../shared/ipc";
import type { ExternalUrl } from "../../../shared/lib";
import { briefingText } from "../messages";
import { linkVerdict } from "../model/briefings";
import { Sources } from "./BriefingSources";

type Scale = "body" | "reading";

export const Body = (props: {
  readonly briefing: BriefingSummary;
  readonly onOpenSource: (url: ExternalUrl) => void;
  readonly scale: Scale;
}): ReactElement => {
  const outcome = props.briefing.outcome;
  switch (outcome.kind) {
    case "completed":
      return (
        <Stack gap="space-3">
          <Markdown
            content={outcome.body}
            citations={briefingCitations(outcome.sources)}
            linkAllowed={(url) => linkVerdict(props.briefing, url)}
            onOpenLink={props.onOpenSource}
            scale={props.scale}
          />
          <Sources
            sources={outcome.sources}
            onOpenSource={props.onOpenSource}
          />
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

export const Stamp = (props: {
  readonly briefing: BriefingSummary;
}): ReactElement => (
  <Inline gap="space-2" align="center" wrap="wrap">
    <Text variant="caption" tone="ink-muted">
      {briefingText("finishedAt", "ko")}
    </Text>
    <TimestampText value={props.briefing.finishedAt} />
  </Inline>
);

export const Reader = (props: {
  readonly briefing: BriefingSummary;
  readonly onOpenSource: (url: ExternalUrl) => void;
  readonly onOpenHistory: (() => void) | undefined;
  readonly onClose: () => void;
}): ReactElement<ComponentProps<typeof Dialog>> => (
  <Dialog
    open="open"
    title={briefingText(props.briefing.kind, "ko")}
    wide="wide"
    onClose={props.onClose}
    actions={
      props.onOpenHistory === undefined
        ? {
            cancel: {
              label: briefingText("closeReader", "ko"),
              onPress: props.onClose,
            },
          }
        : {
            confirm: {
              label: briefingText("openHistory", "ko"),
              onPress: props.onOpenHistory,
            },
            cancel: {
              label: briefingText("closeReader", "ko"),
              onPress: props.onClose,
            },
          }
    }
  >
    <Stamp briefing={props.briefing} />
    <Body
      briefing={props.briefing}
      onOpenSource={props.onOpenSource}
      scale="reading"
    />
  </Dialog>
);
