import { strict as assert } from "node:assert";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { checkSteps } from "../../../harness/check-list/list.mjs";
import {
  absoluteSandbox,
  fill,
  markerReached,
} from "../../../harness/claude-regression/run.mjs";
import {
  ATTEMPT,
  VERDICT,
  coveredR10,
  judge,
  judgeAll,
  overall,
  parseStream,
  resultTable,
  specProblems,
} from "../../../harness/claude-regression/verdict.mjs";
import { repoRoot } from "./paths.mjs";

const readJson = (path) =>
  JSON.parse(readFileSync(join(repoRoot, path), "utf8"));

const spec = readJson("harness/claude-regression/experiments.json");
const recording = readJson("harness/claude-regression/recordings/run.json");

const experiment = (id) => spec.experiments.find((item) => item.id === id);

const removalVariants = (value, path = []) => {
  if (Array.isArray(value)) {
    return value.flatMap((item, index) => [
      {
        path: [...path, index],
        value: [...value.slice(0, index), ...value.slice(index + 1)],
      },
      ...removalVariants(item, [...path, index]).map((variant) => ({
        path: variant.path,
        value: value.map((other, position) =>
          position === index ? variant.value : other,
        ),
      })),
    ]);
  }
  if (typeof value === "object" && value !== null) {
    return Object.keys(value).flatMap((key) => [
      {
        path: [...path, key],
        value: Object.fromEntries(
          Object.entries(value).filter(([other]) => other !== key),
        ),
      },
      ...removalVariants(value[key], [...path, key]).map((variant) => ({
        path: variant.path,
        value: { ...value, [key]: variant.value },
      })),
    ]);
  }
  return [];
};

const recordedIds = recording.rows.map((row) => row.id);

const specVerdict = (candidate) =>
  specProblems(candidate).length === 0 &&
  JSON.stringify(candidate.experiments.map((item) => item.id)) ===
    JSON.stringify(recordedIds) &&
  JSON.stringify(
    judgeAll(candidate.experiments, recording.observed).map((row) => [
      row.id,
      row.verdict,
      row.attempt,
    ]),
  ) ===
    JSON.stringify(
      recording.rows.map((row) => [row.id, row.verdict, row.attempt]),
    );

test("the experiments cover R-10 #1 to #12 and every step the sessions ask for", () => {
  assert.deepEqual(specProblems(spec), []);
  assert.deepEqual(
    coveredR10(spec.experiments),
    Array.from({ length: 12 }, (_, index) => index + 1),
  );
});

test("the recorded run judges every experiment as recorded", () => {
  assert.ok(specVerdict(spec));
  assert.ok(recording.version.length > 0);
});

test("removing any element of the experiment list breaks the list or the recorded verdicts", () => {
  const surviving = removalVariants(spec)
    .filter((variant) => specVerdict(variant.value))
    .map((variant) => variant.path.join("."));
  assert.deepEqual(surviving, []);
});

const toolUse = (id, name, input, parent = null) => ({
  type: "assistant",
  parent_tool_use_id: parent,
  message: { content: [{ type: "tool_use", id, name, input }] },
});

const toolResult = (id, content, parent = null) => ({
  type: "user",
  parent_tool_use_id: parent,
  message: { content: [{ type: "tool_result", tool_use_id: id, content }] },
});

const observedWith = (events, extra = {}) => ({
  sessions: { main: { events, debug: "", markers: {} }, agents: { events } },
  records: [],
  ...extra,
});

test("a denial the hook still gives passes and a missing denial fails", () => {
  const denied = observedWith([
    toolUse("t1", "Bash", { command: "git commit --no-verify -m x" }),
    toolResult("t1", "PreToolUse:Bash hook error: [git-no-verify] 막는다"),
  ]);
  assert.equal(judge(experiment("M6"), denied).verdict, VERDICT.pass);
  const allowed = observedWith([
    toolUse("t1", "Bash", { command: "git commit --no-verify -m x" }),
    toolResult("t1", "[master abc123] x"),
  ]);
  const row = judge(experiment("M6"), allowed);
  assert.equal(row.verdict, VERDICT.fail);
  assert.equal(row.attempt, ATTEMPT.yes);
});

test("an experiment the model did not attempt is unsettled and fails the run with a rerun note", () => {
  const rows = judgeAll(spec.experiments, observedWith([]));
  assert.ok(rows.every((row) => row.verdict !== VERDICT.pass));
  assert.equal(rows.find((row) => row.id === "M6").verdict, VERDICT.unsettled);
  assert.equal(rows.find((row) => row.id === "M6").attempt, ATTEMPT.no);
  const summary = overall(rows);
  assert.equal(summary.passed, false);
  assert.ok(summary.unsettled > 0);
  assert.match(
    resultTable(rows, "2.1.322 (Claude Code)"),
    /미확정은 모델이 시도하지 않은 실험이다\. just claude-regression을 다시 돌린다/u,
  );
});

test("an allowed action that is refused fails", () => {
  const refused = observedWith([
    toolUse("t1", "Write", {
      file_path: "/x/docs/regression-probe.md",
      content: "probe",
    }),
    toolResult("t1", "File created successfully but [protected-path] denied"),
  ]);
  assert.equal(judge(experiment("M1"), refused).verdict, VERDICT.fail);
  const created = observedWith([
    toolUse("t1", "Write", {
      file_path: "/x/docs/regression-probe.md",
      content: "probe",
    }),
    toolResult(
      "t1",
      "File created successfully at: /x/docs/regression-probe.md",
    ),
  ]);
  assert.equal(judge(experiment("M1"), created).verdict, VERDICT.pass);
});

test("subagent probes look only at subagent calls and stop records decide the stop experiments", () => {
  const write = toolUse(
    "s1",
    "Write",
    { file_path: "/wt/crates/domain/src/regression_probe.rs" },
    "a1",
  );
  const mainOnly = observedWith([
    toolUse("s1", "Write", {
      file_path: "/wt/crates/domain/src/regression_probe.rs",
    }),
  ]);
  assert.equal(judge(experiment("S5"), mainOnly).verdict, VERDICT.unsettled);
  const record = (data) => ({
    kind: "implementer",
    name: "implementer-a.json",
    data,
  });
  const blocked = observedWith([write], {
    records: [record({ status: "blocked", blocks: 3 })],
  });
  assert.equal(judge(experiment("S5"), blocked).verdict, VERDICT.pass);
  const short = observedWith([write], {
    records: [record({ status: "failing", blocks: 2 })],
  });
  assert.equal(judge(experiment("S5"), short).verdict, VERDICT.fail);
  assert.equal(
    judge(experiment("S5"), observedWith([write])).verdict,
    VERDICT.unsettled,
  );
});

test("the ConfigChange experiment needs the marker and the blocked line in the debug log", () => {
  const session = (markers, debug) => ({
    sessions: { main: { events: [], debug, markers } },
    records: [],
  });
  assert.equal(
    judge(experiment("M16"), session({}, "")).verdict,
    VERDICT.unsettled,
  );
  assert.equal(
    judge(experiment("M16"), session({ configWritten: true }, "nothing"))
      .verdict,
    VERDICT.fail,
  );
  assert.equal(
    judge(
      experiment("M16"),
      session(
        { configWritten: true },
        "ConfigChange hook blocked change to /x/.claude/settings.local.json",
      ),
    ).verdict,
    VERDICT.pass,
  );
});

test("SendMessage in the tool list fails the tool list experiment", () => {
  const init = (tools) =>
    observedWith([{ type: "system", subtype: "init", tools }]);
  assert.equal(
    judge(experiment("M15"), init(["Bash", "Agent"])).verdict,
    VERDICT.pass,
  );
  assert.equal(
    judge(experiment("M15"), init(["Bash", "SendMessage"])).verdict,
    VERDICT.fail,
  );
});

test("stream parsing skips lines that are not JSON and the marker is a main Bash call", () => {
  const events = parseStream(
    `noise\n${JSON.stringify(toolUse("t1", "Bash", { command: "sleep 20" }))}\n{broken\n`,
  );
  assert.equal(events.length, 1);
  assert.ok(markerReached(events, "sleep 20"));
  assert.ok(
    !markerReached(
      [toolUse("t1", "Bash", { command: "sleep 20" }, "a1")],
      "sleep 20",
    ),
  );
});

test("the locked sandbox gets absolute paths and prompts get the experiment values", () => {
  const sandbox = absoluteSandbox(
    {
      enabled: true,
      filesystem: { denyWrite: ["./justfile", "../wt", "~/x"] },
    },
    "/e/copy",
    "/e",
  );
  assert.deepEqual(sandbox.filesystem.denyWrite, [
    "/e/copy/justfile",
    "/e/wt",
    "~/x",
  ]);
  assert.equal(sandbox.enabled, true);
  assert.equal(
    fill("{copy}/a {base}", { copy: "/c", base: "abc" }),
    "/c/a abc",
  );
});

test("just claude-regression runs the script and is not part of just check", () => {
  const justfile = readFileSync(join(repoRoot, "justfile"), "utf8");
  assert.ok(
    justfile.includes(
      "claude-regression:\n    node harness/claude-regression/run.mjs\n",
    ),
  );
  assert.ok(
    checkSteps(justfile).every(
      (step) => !(step.command ?? "").includes("claude-regression"),
    ),
  );
});
