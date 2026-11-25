import { expect, test } from "vitest";
import { settleSchedules } from "./schedule-call";
import { wire } from "./schema";
import { responses } from "./testing/responses";

test("settleSchedules maps the recorded job list onto the settings screen names", () => {
  const result = settleSchedules({
    status: "ok",
    data: wire(responses.getJobSchedules),
  });
  expect(result.kind).toBe("ok");
  if (result.kind !== "ok") return;
  const morning = result.value.jobs.find(
    (job) => job.job === "morning-briefing",
  );
  expect(morning?.state).toBe("on");
  expect(morning?.time).toBe("08:20");
  expect(result.value.jobs.some((job) => job.job === "us-premarket")).toBe(
    true,
  );
});

test("settleSchedules refuses an unknown job name", () => {
  const [first] = responses.getJobSchedules.jobs;
  expect(
    settleSchedules({
      status: "ok",
      data: wire({ jobs: [{ ...first, job: "unknown-job" }] }),
    }).kind,
  ).toBe("err");
});
