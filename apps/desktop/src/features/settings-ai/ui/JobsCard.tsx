import { useState, type ReactElement } from "react";
import {
  Button,
  Card,
  Inline,
  Stack,
  Switch,
  Text,
  TextField,
} from "../../../shared/ds";
import {
  parseClockTime,
  type ClockTime,
  type ScheduledJobSetting,
  type ScheduledJobs,
} from "../../../shared/ipc";
import { parseInputValue, type InputValue } from "../../../shared/lib";
import { aiText } from "../messages";

type Save = (setting: ScheduledJobSetting) => void;

const TimeField = (props: {
  readonly value: InputValue;
  readonly onChange: (value: InputValue) => void;
  readonly invalid: "valid" | "invalid";
}): ReactElement =>
  props.invalid === "invalid" ? (
    <TextField
      label={aiText("ai.jobTime", "ko")}
      value={props.value}
      onChange={props.onChange}
      error={aiText("ai.jobTimeInvalid", "ko")}
    />
  ) : (
    <TextField
      label={aiText("ai.jobTime", "ko")}
      value={props.value}
      onChange={props.onChange}
    />
  );

const TimeEditor = (props: {
  readonly row: ScheduledJobSetting;
  readonly time: ClockTime;
  readonly save: Save;
}): ReactElement | null => {
  const initial = parseInputValue(props.time);
  const [value, setValue] = useState(
    initial.kind === "ok" ? initial.value : undefined,
  );
  const [invalid, setInvalid] = useState<"valid" | "invalid">("valid");
  if (value === undefined) return null;
  const submit = (): void => {
    const parsed = parseClockTime(value);
    if (parsed.kind === "err") {
      setInvalid("invalid");
      return;
    }
    setInvalid("valid");
    props.save({ ...props.row, time: parsed.value });
  };
  return (
    <Inline gap="space-2" align="end" wrap="wrap">
      <TimeField value={value} onChange={setValue} invalid={invalid} />
      <Button variant="secondary" onClick={submit}>
        {aiText("ai.save", "ko")}
      </Button>
    </Inline>
  );
};

const JobRow = (props: {
  readonly row: ScheduledJobSetting;
  readonly save: Save;
}): ReactElement => (
  <Stack gap="space-2">
    <Switch
      label={aiText(`ai.job.${props.row.job}`, "ko")}
      checked={props.row.state}
      onChange={(state): void => {
        props.save({ ...props.row, state });
      }}
    />
    {props.row.time === null
      ? [
          <Text key="event" variant="body-sm" tone="ink-muted">
            {aiText("ai.jobEvent", "ko")}
          </Text>,
        ]
      : [
          <TimeEditor
            key="time"
            row={props.row}
            time={props.row.time}
            save={props.save}
          />,
        ]}
  </Stack>
);

export const JobsCard = (props: {
  readonly jobs: ScheduledJobs;
  readonly save: Save;
}): ReactElement => (
  <Card title={aiText("ai.jobsTitle", "ko")}>
    <Stack gap="space-4">
      <Text tone="ink-muted">{aiText("ai.jobsBody", "ko")}</Text>
      {props.jobs.jobs.map((row) => (
        <JobRow key={row.job} row={row} save={props.save} />
      ))}
    </Stack>
  </Card>
);
