import { expect, test } from "vitest";
import { settlePortfolio } from "./portfolio-schema";
import { wire } from "./schema";

test("settlePortfolio keeps the coarse failure reason the core attaches", () => {
  expect(
    settlePortfolio({
      status: "error",
      error: wire({ kind: "retryable", reason: "response-mismatch" }),
    }),
  ).toStrictEqual({
    kind: "err",
    error: {
      kind: "command",
      error: { kind: "retryable", reason: "response-mismatch" },
    },
  });
  expect(
    settlePortfolio({
      status: "error",
      error: wire({ kind: "no-data", reason: null }),
    }),
  ).toStrictEqual({
    kind: "err",
    error: { kind: "command", error: { kind: "no-data", reason: null } },
  });
});

test("settlePortfolio refuses an unknown reason instead of guessing", () => {
  expect(
    settlePortfolio({
      status: "error",
      error: wire({ kind: "retryable", reason: "token-leaked" }),
    }),
  ).toStrictEqual({ kind: "err", error: { kind: "framework" } });
});
