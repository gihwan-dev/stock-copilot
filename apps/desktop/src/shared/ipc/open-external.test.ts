import { afterEach, beforeEach, expect, test } from "vitest";
import { parseExternalUrl } from "../lib";
import { openExternal } from "./call";
import {
  answerOpenExternalForExpectedUrl,
  answerOpenExternalWithMalformedResponse,
  installCommandWindow,
  rejectOpenExternalWithErrorObject,
  resetCommands,
  returnRecordedOpenExternalError,
} from "./testing/fakes";

const fixedUrl = "https://claude.ai/settings/data-privacy-controls";

beforeEach(installCommandWindow);
afterEach(resetCommands);

test("openExternal sends the branded url and returns the recorded Rust status", async () => {
  const parsed = parseExternalUrl(fixedUrl);
  expect(parsed.kind).toBe("ok");
  if (parsed.kind === "ok") {
    answerOpenExternalForExpectedUrl(parsed.value);
    await expect(openExternal(parsed.value)).resolves.toStrictEqual({
      kind: "ok",
      value: { status: "opened" },
    });
  }
});

test("openExternal maps a recorded command error to a typed command failure", async () => {
  const parsed = parseExternalUrl(fixedUrl);
  expect(parsed.kind).toBe("ok");
  if (parsed.kind === "ok") {
    returnRecordedOpenExternalError();
    await expect(openExternal(parsed.value)).resolves.toStrictEqual({
      kind: "err",
      error: { kind: "command", error: { kind: "bug" } },
    });
  }
});

test("openExternal maps malformed success payloads to invalid-response", async () => {
  const parsed = parseExternalUrl(fixedUrl);
  expect(parsed.kind).toBe("ok");
  if (parsed.kind === "ok") {
    answerOpenExternalWithMalformedResponse();
    await expect(openExternal(parsed.value)).resolves.toStrictEqual({
      kind: "err",
      error: { kind: "invalid-response" },
    });
  }
});

test("openExternal maps framework rejections to a framework failure without payload", async () => {
  const parsed = parseExternalUrl(fixedUrl);
  expect(parsed.kind).toBe("ok");
  if (parsed.kind === "ok") {
    rejectOpenExternalWithErrorObject();
    await expect(openExternal(parsed.value)).resolves.toStrictEqual({
      kind: "err",
      error: { kind: "framework" },
    });
  }
});
