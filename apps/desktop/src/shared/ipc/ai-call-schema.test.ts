import { expect, test } from "vitest";
import { aiSettingsSchema } from "./ai-schema";
import { responses } from "./testing/responses";

test("the recorded AI settings reply matches the AI screen schema with unknown usage", () => {
  const parsed = aiSettingsSchema.safeParse(responses.getAiSettings);
  expect(parsed.success).toBe(true);
  if (!parsed.success) return;
  expect(parsed.data.tasks).toHaveLength(6);
  expect(parsed.data.usage_percent).toBeNull();
  expect(aiSettingsSchema.safeParse(responses.setBudgetPercent).success).toBe(
    true,
  );
});

test("an AI reply with an unknown model is refused", () => {
  const [first] = responses.getAiSettings.tasks;
  expect(
    aiSettingsSchema.safeParse({
      ...responses.getAiSettings,
      tasks: [{ ...first, model: "gpt" }],
    }).success,
  ).toBe(false);
});
