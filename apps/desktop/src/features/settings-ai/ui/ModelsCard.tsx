import type { ReactElement } from "react";
import { Card, Inline, Select, Stack, Text } from "../../../shared/ds";
import type { AiSettings, AiTaskSetting } from "../../../shared/ipc";
import { aiText } from "../messages";
import type { AiChange } from "../model";
import {
  effortChoices,
  inputOf,
  modelChoices,
  pickEffort,
  pickModel,
} from "./choices";

const TaskRow = (props: {
  readonly row: AiTaskSetting;
  readonly save: (change: AiChange) => void;
}): ReactElement | null => {
  const model = inputOf(props.row.model);
  const effort = inputOf(props.row.effort);
  if (model === undefined || effort === undefined) return null;
  return (
    <Stack gap="space-2">
      <Text variant="label">{aiText(`ai.task.${props.row.task}`, "ko")}</Text>
      <Inline gap="space-3" wrap="wrap">
        <Select
          label={aiText("ai.modelHeader", "ko")}
          options={modelChoices}
          value={model}
          onChange={(value): void => {
            const picked = pickModel(value);
            if (picked !== undefined)
              props.save({
                kind: "task",
                setting: { ...props.row, model: picked },
              });
          }}
        />
        <Select
          label={aiText("ai.effortHeader", "ko")}
          options={effortChoices}
          value={effort}
          onChange={(value): void => {
            const picked = pickEffort(value);
            if (picked !== undefined)
              props.save({
                kind: "task",
                setting: { ...props.row, effort: picked },
              });
          }}
        />
      </Inline>
    </Stack>
  );
};

export const ModelsCard = (props: {
  readonly settings: AiSettings;
  readonly save: (change: AiChange) => void;
}): ReactElement => (
  <Card title={aiText("ai.modelsTitle", "ko")}>
    <Stack gap="space-4">
      <Text tone="ink-muted">{aiText("ai.modelsBody", "ko")}</Text>
      {props.settings.tasks.map((row) => (
        <TaskRow key={row.task} row={row} save={props.save} />
      ))}
    </Stack>
  </Card>
);
