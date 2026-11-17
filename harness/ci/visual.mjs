import { spawnSync } from "node:child_process";
import { resolve } from "node:path";
const root = resolve(import.meta.dirname, "../..");
const install = spawnSync(
  "pnpm",
  ["install", "--frozen-lockfile", "--ignore-scripts"],
  { cwd: root, stdio: "inherit" },
);
if (install.status !== 0) process.exitCode = install.status ?? 1;
else {
  const compare = spawnSync("just", ["visual-check"], {
    cwd: root,
    stdio: "inherit",
  });
  process.exitCode = compare.status ?? 1;
}
