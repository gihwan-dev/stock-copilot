import { preparePlan } from "./mutation-plan.mjs";
import { spawnSync } from "node:child_process";
import { appendFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const git = (root, args) => {
  const run = spawnSync("git", ["--no-replace-objects", ...args], {
    cwd: root,
    encoding: "utf8",
  });
  if (run.status !== 0) throw new Error(`git ${args[0]}: ${run.stderr}`);
  return run.stdout.trim();
};
export const dailyRange = (root, now = new Date()) => {
  if (git(root, ["rev-parse", "--is-shallow-repository"]) === "true")
    git(root, ["fetch", "--unshallow", "origin"]);
  const cutoff = new Date(now.getTime() - 24 * 60 * 60 * 1000).toISOString();
  return (
    git(root, [
      "rev-list",
      "-1",
      "--first-parent",
      `--before=${cutoff}`,
      "HEAD",
    ]) || "initial"
  );
};
export const verifySnapshot = (root, ref, sha) => {
  if (
    ref !== "refs/heads/master" ||
    !/^[0-9a-f]{40}$/u.test(sha) ||
    git(root, ["rev-parse", "HEAD"]) !== sha
  )
    throw new Error("변이 실행은 master의 고정된 실행 SHA가 필요하다");
};
if (process.argv[1] === fileURLToPath(import.meta.url)) {
  if (process.argv[2] === "verify") {
    verifySnapshot(process.cwd(), process.env.REF, process.env.SHA);
  } else if (process.argv[2] === "plan") {
    preparePlan(process.cwd(), dailyRange);
  } else {
    const base = dailyRange(process.cwd());
    process.stdout.write(`지난 24시간 기준: ${base}\n`);
    if (process.env.GITHUB_OUTPUT !== undefined)
      appendFileSync(process.env.GITHUB_OUTPUT, `base=${base}\n`);
  }
}
