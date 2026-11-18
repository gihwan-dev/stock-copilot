import { spawn, spawnSync } from "node:child_process";
import { mkdtempSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { prefixes } from "./temp-prefixes.mjs";

const [command, ...args] = process.argv.slice(2);
if (command === undefined) throw new Error("실행할 검사 명령이 필요하다");
const directory = mkdtempSync(join(tmpdir(), "sc-"));
const compileCache = join(directory, "node-compile-cache");
const before = readdirSync(directory);
const remove = () => rmSync(directory, { recursive: true, force: true });
process.once("exit", remove);
const child = spawn(command, args, {
  env: {
    ...process.env,
    TMPDIR: directory,
    TMP: directory,
    TEMP: directory,
    NODE_COMPILE_CACHE: compileCache,
  },
  stdio: "inherit",
  detached: true,
});
let interrupted;
let inspectionError;
const sessionGroups = () => {
  if (child.pid === undefined) return [];
  const result = spawnSync(
    "python3",
    [
      "-I",
      fileURLToPath(new URL("session-groups.py", import.meta.url)),
      String(child.pid),
    ],
    {
      encoding: "utf8",
    },
  );
  const lines = result.stdout?.trimEnd().split("\n");
  if (
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
    inspectionError = "소유 세션을 확인하지 못했다";
    return undefined;
  }
  const groups = lines.slice(1).map(Number);
  return [...new Set(groups)];
};
const signalSession = (signal) => {
  const groups = sessionGroups();
  if (groups === undefined) {
    if (child.exitCode === null && child.signalCode === null)
      child.kill(signal);
    return;
  }
  for (const group of groups) {
    try {
      process.kill(-group, signal);
    } catch (error) {
      if (error.code !== "ESRCH") throw error;
    }
  }
};
const sessionHasGroups = () => {
  const groups = sessionGroups();
  return groups !== undefined && groups.length > 0;
};
for (const signal of ["SIGINT", "SIGTERM", "SIGHUP"]) {
  process.once(signal, () => {
    interrupted = signal;
    signalSession(signal);
  });
}
child.once("error", (error) => {
  process.stderr.write(`temp-audit: ${error.message}\n`);
  process.exitCode = 1;
});
child.once("close", async (code, signal) => {
  const groups = sessionGroups();
  const descendants = groups === undefined || groups.length > 0;
  if (groups !== undefined && groups.length > 0) {
    process.stderr.write("temp-audit: 종료되지 않은 자손 프로세스가 있다\n");
    signalSession("SIGTERM");
    for (let attempt = 0; attempt < 20 && sessionHasGroups(); attempt += 1)
      await new Promise((done) => setTimeout(done, 50));
    if (sessionHasGroups()) signalSession("SIGKILL");
    for (let attempt = 0; attempt < 20 && sessionHasGroups(); attempt += 1)
      await new Promise((done) => setTimeout(done, 50));
  }
  if (inspectionError !== undefined)
    process.stderr.write(`temp-audit: ${inspectionError}\n`);
  rmSync(compileCache, { recursive: true, force: true });
  rmSync(join(directory, ".pnpm-store"), { recursive: true, force: true });
  const remaining = readdirSync(directory);
  const leftovers = remaining.filter((name) => !before.includes(name));
  for (const name of leftovers) {
    const kind = prefixes.some((prefix) => name.startsWith(prefix))
      ? "repository"
      : "other";
    process.stderr.write(`temp-audit: ${kind}: ${name}\n`);
  }
  process.stdout.write(
    `temp-audit: before=${before.length} after=${remaining.length}\n`,
  );
  remove();
  if (interrupted !== undefined) {
    process.removeAllListeners(interrupted);
    process.kill(process.pid, interrupted);
  } else if (signal !== null) {
    process.exitCode = 1;
  } else {
    process.exitCode =
      code === 0 && (leftovers.length > 0 || descendants) ? 1 : (code ?? 1);
  }
});
