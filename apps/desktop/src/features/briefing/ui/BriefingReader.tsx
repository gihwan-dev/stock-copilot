import type { ReactElement } from "react";
import {
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

export const Body = (props: {
  readonly briefing: BriefingSummary;
  readonly onOpenSource: (url: ExternalUrl) => void;
  readonly presentation: "dashboard" | "full";
}): ReactElement => {
  const outcome = props.briefing.outcome;
  switch (outcome.kind) {
    case "completed": {
      const markdown = (
        <Markdown
          content={outcome.body}
          citations={briefingCitations(outcome.sources)}
          linkAllowed={(url) => linkVerdict(props.briefing, url)}
          onOpenLink={props.onOpenSource}
          scale={props.presentation === "dashboard" ? "body" : "reading"}
        />
      );
      if (props.presentation === "dashboard") return markdown;
      return (
        <Stack gap="space-3">
          {markdown}
          <Sources
            sources={outcome.sources}
            onOpenSource={props.onOpenSource}
          />
          <Text variant="caption" tone="ink-subtle">
            {briefingText("aiNotice", "ko")}
          </Text>
        </Stack>
      );
    }
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
