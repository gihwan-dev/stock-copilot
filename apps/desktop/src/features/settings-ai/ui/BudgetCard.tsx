import type { ReactElement } from "react";
import { Card, Inline, Percent, Select, Stack, Text } from "../../../shared/ds";
import type { AiSettings } from "../../../shared/ipc";
import { parsePercentDisplay } from "../../../shared/lib";
import { aiText } from "../messages";
import type { AiChange } from "../model";
import { budgetChoices, inputOf, pickBudget } from "./choices";

const Usage = (props: { readonly settings: AiSettings }): ReactElement => {
  const usage = parsePercentDisplay(props.settings.usage_percent);
  return (
    <Inline gap="space-2" align="center">
      <Text tone="ink-muted">{aiText("ai.usageLabel", "ko")}</Text>
      {usage.kind === "ok"
        ? [<Percent key="usage" value={usage.value} />]
        : [<Text key="unknown">{aiText("ai.usageUnknown", "ko")}</Text>]}
    </Inline>
  );
};

export const BudgetCard = (props: {
  readonly settings: AiSettings;
  readonly save: (change: AiChange) => void;
}): ReactElement | null => {
  const current = inputOf(props.settings.budget_percent);
  if (current === undefined) return null;
  return (
    <Card title={aiText("ai.budgetTitle", "ko")}>
      <Stack gap="space-3">
        <Select
          label={aiText("ai.budgetLabel", "ko")}
          options={budgetChoices}
          value={current}
          hint={aiText("ai.budgetHint", "ko")}
          onChange={(value): void => {
            const picked = pickBudget(value);
            if (picked !== undefined)
              props.save({ kind: "budget", percent: picked });
          }}
        />
        <Usage settings={props.settings} />
      </Stack>
    </Card>
  );
};
