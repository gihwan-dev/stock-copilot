import { spawn } from "node:child_process";
import { mkdtempSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { prefixes } from "./temp-prefixes.mjs";

const [command, ...args] = process.argv.slice(2);
if (command === undefined) throw new Error("실행할 검사 명령이 필요하다");
const directory = mkdtempSync(join(tmpdir(), "stock-copilot-check-"));
const before = readdirSync(directory);
const remove = () => rmSync(directory, { recursive: true, force: true });
process.once("exit", remove);
const child = spawn(command, args, {
  env: { ...process.env, TMPDIR: directory, TMP: directory, TEMP: directory },
  stdio: "inherit",
  detached: true,
});
let interrupted;
for (const signal of ["SIGINT", "SIGTERM", "SIGHUP"]) {
  process.once(signal, () => {
    interrupted = signal;
    if (child.pid !== undefined) process.kill(-child.pid, signal);
  });
}
child.once("error", (error) => {
  process.stderr.write(`temp-audit: ${error.message}\n`);
  process.exitCode = 1;
});
child.once("close", (code, signal) => {
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
    process.exitCode = code === 0 && leftovers.length > 0 ? 1 : (code ?? 1);
  }
});
