import { spawn } from "node:child_process";
import { mkdtempSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
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
const groupExists = () => {
  if (child.pid === undefined) return false;
  try {
    process.kill(-child.pid, 0);
    return true;
  } catch (error) {
    if (error.code === "ESRCH") return false;
    throw error;
  }
};
const signalGroup = (signal) => {
  if (child.pid === undefined) return;
  try {
    process.kill(-child.pid, signal);
  } catch (error) {
    if (error.code !== "ESRCH") throw error;
  }
};
for (const signal of ["SIGINT", "SIGTERM", "SIGHUP"]) {
  process.once(signal, () => {
    interrupted = signal;
    signalGroup(signal);
  });
}
child.once("error", (error) => {
  process.stderr.write(`temp-audit: ${error.message}\n`);
  process.exitCode = 1;
});
child.once("close", async (code, signal) => {
  const descendants = groupExists();
  if (descendants) {
    process.stderr.write("temp-audit: 종료되지 않은 자손 프로세스가 있다\n");
    signalGroup("SIGTERM");
    for (let attempt = 0; attempt < 20 && groupExists(); attempt += 1)
      await new Promise((done) => setTimeout(done, 50));
    if (groupExists()) signalGroup("SIGKILL");
    for (let attempt = 0; attempt < 20 && groupExists(); attempt += 1)
      await new Promise((done) => setTimeout(done, 50));
  }
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
