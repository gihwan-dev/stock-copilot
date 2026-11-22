import { spawnSync } from "node:child_process";

const git = (args) => spawnSync("git", args, { encoding: "utf8" });
const local = git(["show-ref", "--verify", "--quiet", "refs/heads/master"]);
if (local.status !== 1) {
  process.stderr.write("브랜치 검사에는 로컬 master가 없어야 한다\n");
  process.exitCode = 1;
} else {
  const remote = git([
    "rev-parse",
    "--verify",
    "refs/remotes/origin/master^{commit}",
  ]);
  if (remote.status !== 0 || !/^[0-9a-f]{40}$/u.test(remote.stdout.trim())) {
    process.stderr.write("원격 추적 master를 확인하지 못했다\n");
    process.exitCode = 1;
  }
}
