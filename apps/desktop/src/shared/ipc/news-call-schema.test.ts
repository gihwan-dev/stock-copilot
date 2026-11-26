import { expect, test } from "vitest";
import { newsSettingsSchema } from "./news-schema";
import { responses } from "./testing/responses";

test("the recorded news settings reply matches the news screen schema", () => {
  const parsed = newsSettingsSchema.safeParse(responses.getNewsSettings);
  expect(parsed.success).toBe(true);
  if (!parsed.success) return;
  expect(parsed.data.sources).toHaveLength(14);
  expect(newsSettingsSchema.safeParse(responses.setAliases).success).toBe(true);
});

test("a news reply with an unknown source is refused", () => {
  const [first] = responses.getNewsSettings.sources;
  expect(
    newsSettingsSchema.safeParse({
      ...responses.getNewsSettings,
      sources: [{ ...first, source: "Naver" }],
    }).success,
  ).toBe(false);
});
