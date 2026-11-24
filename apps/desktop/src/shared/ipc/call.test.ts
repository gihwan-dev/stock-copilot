import { afterEach, beforeEach, expect, test } from "vitest";
import { appVersion } from "./call";
import { parseUiEvent } from "./schema";
import {
  advanceCommandTimers,
  answerWithMalformedResponse,
  answerWithRecordedResponses,
  enableFakeCommandTimers,
  hangAppVersion,
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

test("a hanging IPC command resolves to a timeout failure", async () => {
  enableFakeCommandTimers();
  hangAppVersion();
  const pending = appVersion();
  await advanceCommandTimers();
  await expect(pending).resolves.toStrictEqual({
    kind: "err",
    error: { kind: "timeout" },
  });
});

test("parseUiEvent validates unit and keyed core event wire payloads and rejects malformed inputs", () => {
  expect(parseUiEvent("SettingsChanged")).toStrictEqual({
    kind: "ok",
    value: { category: "settings", kind: "SettingsChanged" },
  });
  expect(parseUiEvent("StoredDataChanged")).toStrictEqual({
    kind: "ok",
    value: { category: "all", kind: "StoredDataChanged" },
  });
  expect(parseUiEvent({ FxRateUpdated: "UsdKrw" })).toStrictEqual({
    kind: "ok",
    value: { category: "portfolio", kind: "FxRateUpdated" },
  });
  expect(
    parseUiEvent({ CollectionFinished: { News: "Yonhap" } }),
  ).toStrictEqual({
    kind: "ok",
    value: { category: "status", kind: "CollectionFinished" },
  });
  expect(
    parseUiEvent({
      DisclosureArrived: { source: "Dart", receipt_no: "20261124000001" },
    }),
  ).toStrictEqual({
    kind: "ok",
    value: { category: "activity", kind: "DisclosureArrived" },
  });
  expect(parseUiEvent("UnknownEvent")).toStrictEqual({
    kind: "err",
    error: { kind: "invalid-response" },
  });
  expect(parseUiEvent({ PriceUpdated: "" })).toStrictEqual({
    kind: "err",
    error: { kind: "invalid-response" },
  });
  expect(
    parseUiEvent({ PriceUpdated: "inst-1", PriceMoved: "inst-2" }),
  ).toStrictEqual({
    kind: "err",
    error: { kind: "invalid-response" },
  });
  expect(parseUiEvent(null)).toStrictEqual({
    kind: "err",
    error: { kind: "invalid-response" },
  });
});
