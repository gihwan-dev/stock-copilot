import { afterEach, beforeEach, expect, test } from "vitest";
import { appVersion } from "./call";
import {
  answerWithRecordedResponses,
  installCommandWindow,
  rejectWithFrameworkError,
  resetCommands,
  returnRecordedCommandError,
} from "./testing/fakes";

beforeEach(installCommandWindow);
afterEach(resetCommands);

test("the recorded Rust response becomes a branded app version", async () => {
  answerWithRecordedResponses();
  await expect(appVersion()).resolves.toStrictEqual({
    kind: "ok",
    value: { version: "0.1.0" },
  });
});

test("a recorded command error becomes a typed command failure", async () => {
  returnRecordedCommandError();
  await expect(appVersion()).resolves.toStrictEqual({
    kind: "err",
    error: { kind: "command", error: { kind: "bug" } },
  });
});

test("a free-form framework rejection becomes a framework failure", async () => {
  rejectWithFrameworkError();
  await expect(appVersion()).resolves.toStrictEqual({
    kind: "err",
    error: { kind: "framework" },
  });
});
