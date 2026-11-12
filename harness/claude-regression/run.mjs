import { execFileSync, spawn } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  realpathSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  judgeAll,
  overall,
  parseStream,
  resultTable,
  toolUses,
} from "./verdict.mjs";

const here = resolve(fileURLToPath(import.meta.url), "..");
const repoRoot = resolve(here, "..", "..");
const SESSION_LIMIT_MS = 40 * 60 * 1000;
const CONFIG_PAUSE_MS = 3000;

const run = (command, args, cwd, environment = process.env) =>
  execFileSync(command, args, {
    cwd,
    encoding: "utf8",
    env: environment,
    stdio: ["ignore", "pipe", "pipe"],
  }).trim();

export const fill = (text, values) =>
  Object.entries(values).reduce(
    (current, [key, value]) => current.replaceAll(`{${key}}`, value),
    text,
  );

export const absoluteSandbox = (sandbox, copy, experiment) => {
  const absolute = (path) =>
    path.startsWith("./")
      ? join(copy, path.slice(2))
      : path.startsWith("../")
        ? join(experiment, path.slice(3))
        : path;
  return {
    ...sandbox,
    filesystem: Object.fromEntries(
      Object.entries(sandbox.filesystem).map(([key, paths]) => [
        key,
        paths.map(absolute),
      ]),
    ),
  };
};

const prepare = () => {
  const experiment = realpathSync(
    mkdtempSync(join(tmpdir(), "stock-copilot-regression-")),
  );
  const copy = join(experiment, "stock-copilot");
  const worktree = join(experiment, "stock-copilot-wt", "a");
  const head = run("git", ["rev-parse", "HEAD"], repoRoot);
  run("git", ["clone", "--quiet", repoRoot, copy], experiment);
  run("git", ["checkout", "--quiet", "-B", "master", head], copy);
  run("git", ["remote", "remove", "origin"], copy);
  run(
    "pnpm",
    [
      "install",
      "--frozen-lockfile",
      "--offline",
      "--ignore-scripts",
      "--silent",
    ],
    copy,
  );
  mkdirSync(join(experiment, "stock-copilot-wt"), { recursive: true });
  run("lefthook", ["install"], copy);
  run(
    "git",
    ["worktree", "add", "--quiet", "-b", "wt-a", worktree, "master"],
    copy,
  );
  const reviewerFile = join(copy, "docs", "regression-reviewer.md");
  writeFileSync(
    join(experiment, "reviewer-write.sh"),
    `printf 'x\\n' > '${reviewerFile}'\n`,
  );
  writeFileSync(
    join(experiment, "reviewer-undo.sh"),
    `rm -f '${reviewerFile}'\n`,
  );
  writeFileSync(
    join(experiment, "cargo-config-write.sh"),
    `printf 'x\\n' > '${join(worktree, ".cargo", "config.toml")}'\n`,
  );
  run(
    "cargo",
    ["build", "--locked", "--quiet", "--package", "xtask"],
    worktree,
    { ...process.env, RUSTFLAGS: "-D warnings" },
  );
  const project = JSON.parse(
    readFileSync(join(copy, ".claude", "settings.json"), "utf8"),
  );
  const sandboxFile = join(experiment, "sandbox-settings.json");
  const locked = JSON.parse(
    run("node", ["harness/sandbox/fragment.mjs", "--locked-settings"], copy),
  );
  writeFileSync(
    sandboxFile,
    JSON.stringify({
      sandbox: absoluteSandbox(locked.sandbox, copy, experiment),
    }),
  );
  return {
    experiment,
    copy,
    worktree,
    head,
    state: join(experiment, "state"),
    sandboxFile: project.sandbox?.enabled === true ? undefined : sandboxFile,
  };
};

export const markerReached = (events, marker) =>
  toolUses(events).some(
    (use) =>
      use.name === "Bash" &&
      use.parent === null &&
      String(use.input.command ?? "").includes(marker),
  );

const runSession = (session, prepared) =>
  new Promise((done) => {
    const debugFile = join(prepared.experiment, `${session.id}-debug.log`);
    const prompt = fill(session.prompt.join("\n"), {
      copy: prepared.copy,
      worktree: prepared.worktree,
      base: prepared.head,
      experiment: prepared.experiment,
    });
    const args = [
      "-p",
      "--model",
      "haiku",
      "--setting-sources",
      "project,local",
      "--permission-mode",
      "acceptEdits",
      "--output-format",
      "stream-json",
      "--verbose",
      "--no-session-persistence",
      "--debug-file",
      debugFile,
      "--add-dir",
      join(prepared.experiment, "stock-copilot-wt"),
      ...(prepared.sandboxFile === undefined
        ? []
        : ["--settings", prepared.sandboxFile]),
      prompt,
    ];
    const child = spawn("claude", args, {
      cwd: prepared.copy,
      env: { ...process.env, STOCK_COPILOT_HARNESS_STATE: prepared.state },
      stdio: ["ignore", "pipe", "pipe"],
    });
    const markers = { configWritten: false };
    const localSettings = join(prepared.copy, ".claude", "settings.local.json");
    let output = "";
    const timer = setTimeout(() => child.kill("SIGTERM"), SESSION_LIMIT_MS);
    child.stdout.on("data", (chunk) => {
      output += chunk.toString("utf8");
      if (
        session.configMarker !== undefined &&
        !markers.configWritten &&
        markerReached(parseStream(output), session.configMarker)
      ) {
        markers.configWritten = true;
        setTimeout(() => {
          writeFileSync(
            localSettings,
            `${JSON.stringify({ env: { REGRESSION_PROBE: "1" } })}\n`,
          );
        }, CONFIG_PAUSE_MS);
      }
    });
    child.on("close", () => {
      clearTimeout(timer);
      rmSync(localSettings, { force: true });
      done({
        events: parseStream(output),
        debug: existsSync(debugFile) ? readFileSync(debugFile, "utf8") : "",
        markers,
      });
    });
  });

const readRecords = (state) => {
  const folder = join(state, "records");
  return existsSync(folder)
    ? readdirSync(folder)
        .filter((name) => name.endsWith(".json"))
        .map((name) => ({
          kind: name.slice(0, name.indexOf("-")),
          name,
          data: JSON.parse(readFileSync(join(folder, name), "utf8")),
        }))
    : [];
};

const main = async () => {
  const version = run("claude", ["--version"], repoRoot);
  const spec = JSON.parse(readFileSync(join(here, "experiments.json"), "utf8"));
  const prepared = prepare();
  try {
    const sessions = {};
    for (const session of spec.sessions) {
      process.stdout.write(`claude-regression: 세션 ${session.id} 시작\n`);
      sessions[session.id] = await runSession(session, prepared);
    }
    const observed = { sessions, records: readRecords(prepared.state) };
    const rows = judgeAll(spec.experiments, observed);
    const table = resultTable(rows, version);
    const out = join(
      repoRoot,
      "target",
      "claude-regression",
      new Date().toISOString().replaceAll(":", "-"),
    );
    mkdirSync(out, { recursive: true });
    writeFileSync(
      join(out, "observed.json"),
      `${JSON.stringify({ version, ...observed }, null, 2)}\n`,
    );
    writeFileSync(join(out, "result.md"), `${table}\n`);
    process.stdout.write(`${table}\n기록: ${out}\n`);
    process.exitCode = overall(rows).passed ? 0 : 1;
  } finally {
    rmSync(prepared.experiment, { recursive: true, force: true });
  }
};

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  await main();
}
