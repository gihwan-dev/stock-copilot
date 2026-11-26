import { expect, test } from "vitest";
import { parseFreeText, type FreeTextFold } from "../../../shared/lib";
import { replyFold } from "./view";

type Sample = "short" | "long" | "grid" | "list";

const samples = {
  short: "Keep it. The weight is about 12%.",
  long: "One. Two! Three? Four. Five. Six.",
  grid: "Plan. A. B. C. D.\n\n| Item | Action |\n|---|---|\n| A | Buy |",
  list: "Plan. A. B. C. D.\n\n- Watch",
} as const;

const fold = (sample: Sample): FreeTextFold | "err" => {
  const parsed = parseFreeText(samples[sample]);
  return parsed.kind === "ok" ? replyFold(parsed.value) : "err";
};

test("short prose answers stay whole", () => {
  expect(fold("short")).toStrictEqual({ kind: "whole" });
});

test("prose longer than four sentences folds after the fourth", () => {
  expect(fold("long")).toStrictEqual({
    kind: "folded",
    head: "One. Two! Three? Four.",
  });
});

test("structured plans with tables or lists are never folded", () => {
  expect(fold("grid")).toStrictEqual({ kind: "whole" });
  expect(fold("list")).toStrictEqual({ kind: "whole" });
});
