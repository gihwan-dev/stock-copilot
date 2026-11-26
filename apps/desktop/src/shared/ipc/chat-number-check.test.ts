import { expect, test } from "vitest";
import { parseChatStreamEvent } from "./chat-schema";
import { wire } from "./schema";

type Status = "NoNumbers" | "Verified" | "Computed" | "Unverified" | "Guessed";

const numberCheck = (status: Status): ReturnType<typeof parseChatStreamEvent> =>
  parseChatStreamEvent(
    wire({
      sequence: 7,
      turn_id: "turn-1",
      chunk: { NumberCheck: { status } },
    }),
  );

test("number check statuses map to the four reply marks, computed included", () => {
  expect(
    (["NoNumbers", "Verified", "Computed", "Unverified"] as const).map(
      (status) => {
        const parsed = numberCheck(status);
        return parsed.kind === "ok" && parsed.value.chunk.kind === "content"
          ? parsed.value.chunk.content
          : parsed.kind;
      },
    ),
  ).toStrictEqual([
    { kind: "number-check", check: "no-numbers" },
    { kind: "number-check", check: "verified" },
    { kind: "number-check", check: "computed" },
    { kind: "number-check", check: "unverified" },
  ]);
});

test("an unknown number check status is rejected", () => {
  expect(numberCheck("Guessed").kind).toBe("err");
});
