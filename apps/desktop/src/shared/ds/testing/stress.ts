import { stressMessage } from "./messages";
import { parseEntityId } from "../../lib";

export const stressFixtures = {
  company: stressMessage("company", "ko"),
  english: stressMessage("english", "ko"),
  paragraph: stressMessage("paragraph", "ko"),
  money: "9876543210123",
  quantity: "987654321.12345678",
  digits: "12345678901234567890123456789012345678901234567890",
  empty: stressMessage("empty", "ko"),
  holdings: Array.from(Array.from({ length: 200 }).keys(), (index) => {
    const id = parseEntityId(`holding-${index.toString()}`);
    return id.kind === "ok" ? [id.value] : [];
  }).flat(),
  paragraphs: Array.from({ length: 20 }, () =>
    stressMessage("paragraph", "ko"),
  ),
  sources: Array.from(
    Array.from({ length: 30 }).keys(),
    (index) => `https://example.test/source-${index.toString()}`,
  ),
} as const;
