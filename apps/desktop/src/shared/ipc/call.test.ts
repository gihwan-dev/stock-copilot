import { afterEach, beforeEach, expect, test } from "vitest";
import { appVersion } from "./call";
import {
  answerWithMalformedResponse,
  answerWithRecordedResponses,
  installCommandWindow,
  rejectWithErrorObject,
  rejectWithUndefined,
  rejectWithUnknownObject,
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

test("a response that does not match the schema becomes an invalid-response failure", async () => {
  answerWithMalformedResponse();
  await expect(appVersion()).resolves.toStrictEqual({
    kind: "err",
    error: { kind: "invalid-response" },
  });
});

test("an Error rejection becomes a framework failure without its message", async () => {
  rejectWithErrorObject();
  await expect(appVersion()).resolves.toStrictEqual({
    kind: "err",
    error: { kind: "framework" },
  });
});

test("an object rejection that is not a command error becomes a framework failure", async () => {
  rejectWithUnknownObject();
  await expect(appVersion()).resolves.toStrictEqual({
    kind: "err",
    error: { kind: "framework" },
  });
});

test("an undefined rejection becomes a framework failure", async () => {
  rejectWithUndefined();
  await expect(appVersion()).resolves.toStrictEqual({
    kind: "err",
    error: { kind: "framework" },
  });
});
