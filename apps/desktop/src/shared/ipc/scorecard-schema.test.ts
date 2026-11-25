import { expect, test } from "vitest";
import { text } from "../lib";
import { wire } from "./schema";
import { settleScorecard } from "./scorecard-schema";
import { responses } from "./testing/responses";

test("settleScorecard reads an empty scorecard", () => {
  expect(
    settleScorecard({ status: "ok", data: wire(responses.getScorecard) }),
  ).toStrictEqual({ kind: "ok", value: { combinations: [] } });
});

test("settleScorecard labels horizons and bins from messages and drops empty bins", () => {
  const result = settleScorecard({
    status: "ok",
    data: wire({
      combinations: [
        {
          model: "claude-opus",
          horizon: "few-days",
          judged: "31",
          verdict: "ready",
          brier: "0.2100",
          comparisons: "2",
          bins: [
            {
              bin: "below-50",
              count: "0",
              mean_prediction: null,
              hit_rate: null,
            },
            {
              bin: "60-70",
              count: "12",
              mean_prediction: "0.6400",
              hit_rate: "0.5833",
            },
          ],
        },
      ],
    }),
  });
  expect(result.kind).toBe("ok");
  if (result.kind === "ok") {
    const [card] = result.value.combinations;
    expect(card?.horizon).toBe(text("horizon.few-days", "ko"));
    expect(card?.bins.map((bin) => bin.label)).toStrictEqual([
      text("bin.60-70", "ko"),
    ]);
    expect(card?.brier).toBe("0.2100");
  }
  expect(
    settleScorecard({
      status: "ok",
      data: wire({ combinations: [{ model: "m", horizon: "years" }] }),
    }).kind,
  ).toBe("err");
});
