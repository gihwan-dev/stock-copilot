import type { ReactElement } from "react";
import {
  Badge,
  Card,
  DataText,
  EmptyState,
  Inline,
  Skeleton,
  Stack,
  Text,
  TimestampText,
} from "../../../shared/ds";
import type { ScreenSources, ThesisView } from "../../../shared/ipc";
import type { EntityId } from "../../../shared/lib";
import { instrumentsText } from "../messages";
import { useThesis } from "../model/instruments";

const ThesisReady = (props: { readonly view: ThesisView }): ReactElement => (
  <Stack gap="space-3">
    <Text variant="label">{instrumentsText("thesisCurrent", "ko")}</Text>
    <Stack gap="space-2">
      {props.view.claims.map((claim): ReactElement => (
        <Inline key={claim.id} gap="space-2" align="center" wrap="wrap">
          <Badge tone={claim.status === "confirmed" ? "neutral" : "alert"}>
            {instrumentsText(
              claim.status === "confirmed" ? "confirmed" : "needsReview",
              "ko",
            )}
          </Badge>
          <DataText value={claim.text} variant="body-sm" />
        </Inline>
      ))}
    </Stack>
    <Text variant="label">{instrumentsText("thesisHistory", "ko")}</Text>
    <Stack gap="space-2">
      {props.view.history.map((revision): ReactElement => (
        <Inline key={revision.id} gap="space-2" align="center" wrap="wrap">
          {revision.at === undefined ? (
            <Text variant="caption" tone="ink-muted">
              {instrumentsText("thesisTimeUnknown", "ko")}
            </Text>
          ) : (
            <TimestampText value={revision.at} variant="caption" />
          )}
          <DataText value={revision.summary} variant="body-sm" />
        </Inline>
      ))}
    </Stack>
  </Stack>
);

const ThesisBody = (props: {
  readonly load: ScreenSources["thesis"];
  readonly instrument: EntityId;
}): ReactElement => {
  const thesis = useThesis(props.load, props.instrument);
  switch (thesis.view.status) {
    case "loading":
      return (
        <Stack gap="space-2">
          <Text variant="caption">
            {instrumentsText("thesisLoading", "ko")}
          </Text>
          <Skeleton variant="text" lines={3} />
        </Stack>
      );
    case "empty":
      return (
        <Text variant="body-sm" tone="ink-muted">
          {instrumentsText("thesisEmpty", "ko")}
        </Text>
      );
    case "error":
      return (
        <EmptyState
          title={instrumentsText("thesisError", "ko")}
          action={{
            label: instrumentsText("retry", "ko"),
            onPress: thesis.retry,
          }}
        />
      );
    case "ready":
      return <ThesisReady view={thesis.view.data} />;
  }
};

export const ThesisNotes = (props: {
  readonly load: ScreenSources["thesis"];
  readonly instrument: EntityId;
}): ReactElement => (
  <Card title={instrumentsText("thesisTitle", "ko")}>
    <ThesisBody load={props.load} instrument={props.instrument} />
  </Card>
);
