import { expect, test } from "vitest";
import { text } from "../shared/lib";
import { createAppQueryClient, describeIpcFailure } from "./query";

test("describeIpcFailure maps every IPC failure kind to user-facing text", () => {
  expect(describeIpcFailure({ kind: "timeout" })).toBe(
    text("app.ipcTimeout", "ko"),
  );
  expect(describeIpcFailure({ kind: "framework" })).toBe(
    text("app.ipcUnavailable", "ko"),
  );
  expect(describeIpcFailure({ kind: "invalid-response" })).toBe(
    text("app.ipcUnavailable", "ko"),
  );
  expect(
    describeIpcFailure({ kind: "command", error: { kind: "retryable" } }),
  ).toBe(text("app.ipcRetryable", "ko"));
  expect(
    describeIpcFailure({ kind: "command", error: { kind: "user-fixable" } }),
  ).toBe(text("app.ipcUserFixable", "ko"));
  expect(describeIpcFailure({ kind: "command", error: { kind: "bug" } })).toBe(
    text("app.ipcBug", "ko"),
  );
  expect(
    describeIpcFailure({ kind: "command", error: { kind: "no-data" } }),
  ).toBe(text("app.ipcNoData", "ko"));
});

test("createAppQueryClient disables implicit retries and focus refetches", () => {
  const client = createAppQueryClient();
  expect(client.getDefaultOptions().queries?.retry).toBe(false);
  expect(client.getDefaultOptions().queries?.refetchOnWindowFocus).toBe(false);
});
