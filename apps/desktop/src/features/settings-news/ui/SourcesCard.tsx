import type { ReactElement } from "react";
import { Card, Inline, Select, Stack, Switch, Text } from "../../../shared/ds";
import type { SourceSetting } from "../../../shared/ipc";
import { newsText } from "../messages";
import type { NewsChange } from "../model";
import { inputOf, intervalChoices, pickInterval } from "./choices";

type Save = (change: NewsChange) => void;

const BodySwitch = (props: {
  readonly row: SourceSetting;
  readonly save: Save;
}): ReactElement =>
  props.row.body_reading === "locked" ? (
    <Switch
      label={newsText("news.bodyReading", "ko")}
      description={newsText("news.bodyLocked", "ko")}
      checked="off"
      disabled="disabled"
    />
  ) : (
    <Switch
      label={newsText("news.bodyReading", "ko")}
      description={newsText("news.bodyHint", "ko")}
      checked={props.row.body_reading}
      onChange={(state): void => {
        props.save({
          kind: "source",
          setting: { ...props.row, body_reading: state },
        });
      }}
    />
  );

const SourceRow = (props: {
  readonly row: SourceSetting;
  readonly save: Save;
}): ReactElement | null => {
  const interval = inputOf(props.row.interval_minutes);
  if (interval === undefined) return null;
  return (
    <Stack gap="space-2">
      <Text variant="label">
        {newsText(`news.source.${props.row.source}`, "ko")}
      </Text>
      <Inline gap="space-4" align="center" wrap="wrap">
        <Switch
          label={newsText("news.sourceOn", "ko")}
          checked={props.row.state}
          onChange={(state): void => {
            props.save({ kind: "source", setting: { ...props.row, state } });
          }}
        />
        <BodySwitch row={props.row} save={props.save} />
        <Select
          label={newsText("news.interval", "ko")}
          options={intervalChoices}
          value={interval}
          onChange={(value): void => {
            const picked = pickInterval(value);
            if (picked !== undefined)
              props.save({
                kind: "source",
                setting: { ...props.row, interval_minutes: picked },
              });
          }}
        />
      </Inline>
    </Stack>
  );
};

export const SourcesCard = (props: {
  readonly sources: readonly SourceSetting[];
  readonly save: Save;
}): ReactElement => (
  <Card title={newsText("news.sourcesTitle", "ko")}>
    <Stack gap="space-4">
      {props.sources.map((row) => (
        <SourceRow key={row.source} row={row} save={props.save} />
      ))}
    </Stack>
  </Card>
);
