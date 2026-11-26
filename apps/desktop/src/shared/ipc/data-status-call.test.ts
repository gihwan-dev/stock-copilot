import { expect, test } from "vitest";
import { settleDataStatus } from "./data-status-call";
import { wire } from "./schema";
import { responses } from "./testing/responses";

test("settleDataStatus keeps last success, latest run and next run per job", () => {
  const result = settleDataStatus({
    status: "ok",
    data: wire(responses.getDataStatus),
  });
  expect(result.kind).toBe("ok");
  if (result.kind === "ok") {
    expect(result.value.jobs.map((row) => row.job)).toStrictEqual([
      "holdings",
      "transactions",
      "backup",
    ]);
    const holdings = result.value.jobs[0];
    expect(holdings?.lastSuccess).toBe("2026-11-24T08:25:00+09:00");
    expect(holdings?.lastRun?.outcome).toBe("failed");
    expect(holdings?.nextRun).toBe("2026-11-25T08:20:00+09:00");
    expect(result.value.jobs[2]?.nextRun).toBeUndefined();
    expect(result.value.jobs[1]?.lastRun).toBeUndefined();
  }
  expect(
    settleDataStatus({
      status: "ok",
      data: wire({
        jobs: [
          { job: "x", last_success: null, last_run: null, next_run: null },
        ],
      }),
    }).kind,
  ).toBe("err");
});
