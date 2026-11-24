import { spawn, spawnSync } from "node:child_process";
import { mkdtempSync, readdirSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { prefixes } from "./temp-prefixes.mjs";

const SESSION_INSPECTION_ERROR = "소유 세션을 확인하지 못했다";
const SESSION_GROUPS_SCRIPT = fileURLToPath(
  new URL("session-groups.py", import.meta.url),
);

export const parseSessionGroupsOutput = (result) => {
  const lines = result.stdout?.trimEnd().split("\n");
  if (
    result.error !== undefined ||
    result.status !== 0 ||
    lines?.[0] !== "ok" ||
    lines
      .slice(1)
      .some(
        (line) =>
          !/^\d+$/u.test(line) ||
          !Number.isSafeInteger(Number(line)) ||
          Number(line) <= 0,
      )
  ) {
    return undefined;
  }
  const groups = lines.slice(1).map(Number);
  return [...new Set(groups)];
};

export const querySessionGroups = (pid) =>
  parseSessionGroupsOutput(
    spawnSync("python3", ["-I", SESSION_GROUPS_SCRIPT, String(pid)], {
      encoding: "utf8",
    }),
  );

const defaultSignalGroup = (group, signal) => {
  try {
    process.kill(-group, signal);
  } catch (error) {
    if (error.code !== "ESRCH") throw error;
  }
};

const defaultWait = (ms) => new Promise((done) => setTimeout(done, ms));

export const runTempAudit = (command, args, options = {}) => {
  if (command === undefined) throw new Error("실행할 검사 명령이 필요하다");
  const {
    baseTempDir = tmpdir(),
    env = process.env,
    stdio = "inherit",
    stdout = process.stdout,
    stderr = process.stderr,
    inspectSessionGroups = querySessionGroups,
    signalGroup = defaultSignalGroup,
    wait = defaultWait,
    pollDelayMs = 50,
    handleSignals = false,
  } = options;

  const directory = mkdtempSync(join(baseTempDir, "sc-"));
  const compileCache = join(directory, "node-compile-cache");
  const before = readdirSync(directory);
  const remove = () => rmSync(directory, { recursive: true, force: true });
  process.once("exit", remove);

  const child = spawn(command, args, {
    env: {
      ...env,
      TMPDIR: directory,
      TMP: directory,
      TEMP: directory,
      NODE_COMPILE_CACHE: compileCache,
    },
    stdio,
    detached: true,
  });

  let interrupted;
  let inspectionError;

  const sessionGroups = () => {
    if (child.pid === undefined) return [];
    const groups = inspectSessionGroups(child.pid);
    if (groups === undefined) {
      inspectionError = SESSION_INSPECTION_ERROR;
      return undefined;
    }
    return groups;
  };

  const signalSession = (signal) => {
    const groups = sessionGroups();
    if (groups === undefined) {
      if (child.exitCode === null && child.signalCode === null)
        child.kill(signal);
      return;
    }
    for (const group of groups) signalGroup(group, signal);
  };

  const sessionHasGroups = () => {
    const groups = sessionGroups();
    return groups !== undefined && groups.length > 0;
  };

  const signalHandlers = [];
  if (handleSignals) {
    for (const signal of ["SIGINT", "SIGTERM", "SIGHUP"]) {
      const handler = () => {
        interrupted = signal;
        signalSession(signal);
      };
      signalHandlers.push([signal, handler]);
      process.once(signal, handler);
    }
  }

  const cleanupListeners = () => {
    for (const [signal, handler] of signalHandlers)
      process.removeListener(signal, handler);
    process.removeListener("exit", remove);
  };

  return new Promise((done) => {
    let settled = false;
    const finish = (outcome) => {
      if (settled) return;
      settled = true;
      cleanupListeners();
      remove();
      done(outcome);
    };

    child.once("error", (error) => {
      stderr.write(`temp-audit: ${error.message}\n`);
      finish({ exitCode: 1, interrupted });
    });

    child.once("close", async (code, signal) => {
      const groups = sessionGroups();
      const descendants = groups === undefined || groups.length > 0;
      if (groups !== undefined && groups.length > 0) {
        stderr.write("temp-audit: 종료되지 않은 자손 프로세스가 있다\n");
        signalSession("SIGTERM");
        for (let attempt = 0; attempt < 20 && sessionHasGroups(); attempt += 1)
          await wait(pollDelayMs);
        if (sessionHasGroups()) signalSession("SIGKILL");
        for (let attempt = 0; attempt < 20 && sessionHasGroups(); attempt += 1)
          await wait(pollDelayMs);
      }
      if (inspectionError !== undefined)
        stderr.write(`temp-audit: ${inspectionError}\n`);
      rmSync(compileCache, { recursive: true, force: true });
      rmSync(join(directory, ".pnpm-store"), { recursive: true, force: true });
      const remaining = readdirSync(directory);
      const leftovers = remaining.filter((name) => !before.includes(name));
      for (const name of leftovers) {
        const kind = prefixes.some((prefix) => name.startsWith(prefix))
          ? "repository"
          : "other";
        stderr.write(`temp-audit: ${kind}: ${name}\n`);
      }
      stdout.write(
        `temp-audit: before=${before.length} after=${remaining.length}\n`,
      );
      const exitCode =
        interrupted !== undefined
          ? undefined
          : signal !== null
            ? 1
            : code === 0 && (leftovers.length > 0 || descendants)
              ? 1
              : (code ?? 1);
      finish({ exitCode, interrupted });
    });
  });
};

const isDirectExecution = (() => {
  if (process.argv[1] === undefined) return false;
  try {
    return (
      realpathSync(resolve(process.argv[1])) ===
      realpathSync(fileURLToPath(import.meta.url))
    );
  } catch {
    return resolve(process.argv[1]) === fileURLToPath(import.meta.url);
  }
})();

if (isDirectExecution) {
  const [command, ...args] = process.argv.slice(2);
  const outcome = await runTempAudit(command, args, { handleSignals: true });
  if (outcome.interrupted !== undefined) {
    process.kill(process.pid, outcome.interrupted);
  } else {
    process.exitCode = outcome.exitCode;
  }
}
