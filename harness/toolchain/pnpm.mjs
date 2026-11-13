import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

export const pnpmVersion = (manifest) => {
  const match = /^pnpm@([^+]+)(?:\+.*)?$/u.exec(manifest.packageManager ?? "");
  if (match === null) throw new Error("packageManager에 고정 pnpm 버전이 없다");
  return match[1];
};

export const checkPnpm = (manifest, run) => {
  const expected = pnpmVersion(manifest);
  const result = run();
  const actual = result.stdout?.trim();
  return result.status === 0 && actual === expected
    ? undefined
    : `check-pnpm: pnpm ${expected} 필요, 실제 ${actual || "실행 실패"}. PATH 앞에 독립 설치본($HOME/Library/pnpm)을 두고 which -a pnpm corepack node로 확인하라. 사용자 설정과 /usr/local/bin을 바꾸지 않는다.\n${result.stderr ?? ""}`;
};

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const root = join(dirname(fileURLToPath(import.meta.url)), "../..");
  const problem = checkPnpm(
    JSON.parse(readFileSync(join(root, "package.json"), "utf8")),
    () => spawnSync("pnpm", ["--version"], { encoding: "utf8" }),
  );
  if (problem !== undefined) {
    process.stderr.write(`${problem}\n`);
    process.exitCode = 1;
  }
}
