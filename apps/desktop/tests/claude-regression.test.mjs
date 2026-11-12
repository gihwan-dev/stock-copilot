import { strict as assert } from "node:assert";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { checkSteps, dumpJustfile } from "../../../harness/check-list/list.mjs";
import {
  absoluteSandbox,
  fill,
  markerReached,
  prepare,
  snapshotTargets,
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
  toolUses,
} from "../../../harness/claude-regression/verdict.mjs";
import { repoRoot } from "./paths.mjs";

const readJson = (path) =>
  JSON.parse(readFileSync(join(repoRoot, path), "utf8"));

const spec = readJson("harness/claude-regression/experiments.json");
const recording = readJson("harness/claude-regression/recordings/run.json");
const recorded = recording.observed;

const experiment = (id) => spec.experiments.find((item) => item.id === id);

const rowKey = (rows) =>
  JSON.stringify(
    rows.map((row) => [row.id, row.verdict, row.attempt, row.checked]),
  );

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

const specHolds = (candidate) =>
  specProblems(candidate).length === 0 &&
  JSON.stringify(candidate.setup) === JSON.stringify(recording.setup) &&
  JSON.stringify(candidate.experiments.map((item) => item.id)) ===
    JSON.stringify(recording.rows.map((row) => row.id)) &&
  JSON.stringify(snapshotTargets(candidate.experiments, recorded.values)) ===
    JSON.stringify(Object.keys(recorded.snapshots.before)) &&
  rowKey(judgeAll(candidate.experiments, recorded)) === rowKey(recording.rows);

const withSession = (id, change) => ({
  ...recorded,
  sessions: { ...recorded.sessions, [id]: change(recorded.sessions[id]) },
});

const usesOf = (id) => {
  const probe = experiment(id).probe;
  return toolUses(recorded.sessions[probe.session].events).filter(
    (use) =>
      new RegExp(probe.tool, "u").test(use.name) &&
      new RegExp(probe.input, "u").test(JSON.stringify(use.input)),
  );
};

const attempt = (id, name, input, content, error, parent = null) => [
  {
    type: "assistant",
    parent_tool_use_id: parent,
    message: { content: [{ type: "tool_use", id, name, input }] },
  },
  {
    type: "user",
    parent_tool_use_id: parent,
    message: {
      content: [
        { type: "tool_result", tool_use_id: id, is_error: error, content },
      ],
    },
  },
];

const replaceResult = (events, useId, content, error) =>
  events.map((event) => ({
    ...event,
    message:
      event.message === undefined
        ? undefined
        : {
            ...event.message,
            content: event.message.content.map((part) =>
              part.type === "tool_result" && part.tool_use_id === useId
                ? { ...part, content, is_error: error }
                : part,
            ),
          },
  }));

const replaceInput = (events, useId, input) =>
  events.map((event) => ({
    ...event,
    message:
      event.message === undefined
        ? undefined
        : {
            ...event.message,
            content: event.message.content.map((part) =>
              part.type === "tool_use" && part.id === useId
                ? { ...part, input }
                : part,
            ),
          },
  }));

test("the experiments cover R-10 #1 to #12 and every step the sessions ask for", () => {
  assert.deepEqual(specProblems(spec), []);
  assert.deepEqual(
    coveredR10(spec.experiments),
    Array.from({ length: 12 }, (_, index) => index + 1),
  );
});

test("the recorded run judges every experiment as recorded", () => {
  assert.ok(specHolds(spec));
  assert.ok(recording.version.length > 0);
});

test("removing any element of the experiment list breaks the list or the recorded verdicts", () => {
  const surviving = removalVariants(spec)
    .filter((variant) => specHolds(variant.value))
    .map((variant) => variant.path.join("."));
  assert.deepEqual(surviving, []);
});

test("recording: a denied attempt followed by a retry that gets through fails the experiment", () => {
  const [use] = usesOf("M6");
  assert.equal(judge(experiment("M6"), recorded).verdict, VERDICT.pass);
  const retried = withSession("main", (session) => ({
    ...session,
    events: [
      ...session.events,
      ...attempt(
        "retry",
        "Bash",
        { command: "git commit --no-verify --allow-empty -m again" },
        "[master 1234567] again",
        false,
      ),
    ],
  }));
  assert.ok(use !== undefined);
  assert.equal(judge(experiment("M6"), retried).verdict, VERDICT.fail);
  const leaked = {
    ...recorded,
    snapshots: {
      ...recorded.snapshots,
      after: {
        ...recorded.snapshots.after,
        [fill("{copy}/justfile", recorded.values)]: "changed",
      },
    },
  };
  assert.equal(judge(experiment("M4"), recorded).verdict, VERDICT.pass);
  const row = judge(experiment("M4"), leaked);
  assert.equal(row.verdict, VERDICT.fail);
  assert.match(row.actual, /대상이 바뀌었다/u);
});

test("recording: a network that is down altogether leaves the network experiment unsettled", () => {
  assert.equal(judge(experiment("M10"), recorded).verdict, VERDICT.pass);
  const control = toolUses(recorded.sessions.main.events).find((use) =>
    String(use.input.command ?? "").includes("registry.npmjs.org"),
  );
  const down = withSession("main", (session) => ({
    ...session,
    events: replaceResult(session.events, control.id, "status 000", true),
  }));
  assert.equal(judge(experiment("M10"), down).verdict, VERDICT.unsettled);
  const forbidden = toolUses(recorded.sessions.main.events).find((use) =>
    String(use.input.command ?? "").includes("example.com"),
  );
  const open = withSession("main", (session) => ({
    ...session,
    events: replaceResult(session.events, forbidden.id, "status 200", false),
  }));
  assert.equal(judge(experiment("M10"), open).verdict, VERDICT.fail);
});

test("recording: a command that prints the denial text itself is not a denial", () => {
  const [use] = usesOf("M9");
  const forged = withSession("main", (session) => ({
    ...session,
    events: replaceResult(
      replaceInput(session.events, use.id, {
        command:
          "ls ~/Library/Keychains; echo 'ls: Operation not permitted'; false",
      }),
      use.id,
      "Exit code 1\nls: Operation not permitted",
      true,
    ),
  }));
  assert.equal(judge(experiment("M9"), forged).verdict, VERDICT.fail);
  const [hookUse] = usesOf("M6");
  const printed = withSession("main", (session) => ({
    ...session,
    events: replaceResult(
      session.events,
      hookUse.id,
      "Exit code 1\nPreToolUse:Bash hook error: [git-no-verify] printed",
      true,
    ),
  }));
  assert.equal(judge(experiment("M6"), printed).verdict, VERDICT.fail);
});

test("recording: an experiment the model did not attempt is unsettled and fails the run with a rerun note", () => {
  const [use] = usesOf("M6");
  const skipped = withSession("main", (session) => ({
    ...session,
    events: session.events.filter(
      (event) => !JSON.stringify(event.message?.content ?? []).includes(use.id),
    ),
  }));
  const rows = judgeAll(spec.experiments, skipped);
  const row = rows.find((item) => item.id === "M6");
  assert.equal(row.verdict, VERDICT.unsettled);
  assert.equal(row.attempt, ATTEMPT.no);
  assert.equal(overall(rows).passed, false);
  assert.match(
    resultTable(rows, recording.version),
    /just claude-regression을 다시 돌린다/u,
  );
});

test("recording: the stop experiments tell the three block reasons apart", () => {
  ["S5", "S9", "S10"].forEach((id) =>
    assert.equal(judge(experiment(id), recorded).verdict, VERDICT.pass, id),
  );
  const swapped = (id, other) => ({
    ...experiment(id),
    evidence: experiment(other).evidence,
    forbid: experiment(other).forbid,
  });
  assert.equal(judge(swapped("S5", "S9"), recorded).verdict, VERDICT.fail);
  assert.equal(judge(swapped("S9", "S10"), recorded).verdict, VERDICT.fail);
  assert.equal(judge(swapped("S10", "S5"), recorded).verdict, VERDICT.fail);
});

test("SendMessage in the tool list fails the tool list experiment", () => {
  const init = (tools) => ({
    ...recorded,
    sessions: {
      ...recorded.sessions,
      main: {
        ...recorded.sessions.main,
        events: [{ type: "system", subtype: "init", tools }],
      },
    },
  });
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
    `noise\n${JSON.stringify(attempt("t1", "Bash", { command: "sleep 20" }, "", false)[0])}\n{broken\n`,
  );
  assert.equal(events.length, 1);
  assert.ok(markerReached(events, "sleep 20"));
  assert.ok(
    !markerReached(
      [attempt("t1", "Bash", { command: "sleep 20" }, "", false, "a1")[0]],
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

test("a failing preparation removes its temporary folder", () => {
  const seen = [];
  assert.throws(() =>
    prepare(spec.setup, (folder) => {
      seen.push(folder);
      throw new Error("preparation failed");
    }),
  );
  assert.equal(seen.length, 1);
  assert.equal(existsSync(seen[0]), false);
});

test("just claude-regression runs the script and is not part of just check", () => {
  const justfile = readFileSync(join(repoRoot, "justfile"), "utf8");
  assert.ok(
    justfile.includes(
      "claude-regression:\n    node harness/claude-regression/run.mjs\n",
    ),
  );
  assert.ok(
    checkSteps(dumpJustfile(join(repoRoot, "justfile"))).every(
      (step) => !(step.command ?? "").includes("claude-regression"),
    ),
  );
});
