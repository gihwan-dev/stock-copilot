import { homedir } from "node:os";
import { spawnSync } from "node:child_process";
import { writeFileSync, mkdirSync, rmSync, existsSync } from "node:fs";
import { mkdtempSync } from "../ci/managed-temp.mjs";
import { resolve, join } from "node:path";
import { settings } from "./policy.mjs";
const root = resolve(import.meta.dirname, "../..");
const mode = process.argv[2] ?? "compare";
if (!["measure", "update", "compare"].includes(mode))
  throw new Error("measure/update/compare 필요");
const output = join(root, "target/DS-15");
mkdirSync(output, { recursive: true });
mkdirSync(join(root, "harness/visual/baselines"), { recursive: true });
const config = mkdtempSync(join(output, "docker-anonymous-"));
const run = (command, args, options = {}) => {
  const result = spawnSync(command, args, {
    cwd: root,
    stdio: "inherit",
    ...options,
  });
  if (result.status !== 0) throw new Error(`${command} 실패: ${result.status}`);
  return result.stdout;
};
try {
  writeFileSync(join(config, "config.json"), "{}\n");
  const socket = [
    "/var/run/docker.sock",
    join(homedir(), ".colima/default/docker.sock"),
    join(homedir(), ".docker/run/docker.sock"),
  ].find(existsSync);
  const host =
    process.env.DOCKER_HOST ??
    (socket === undefined ? undefined : `unix://${socket}`);
  const docker = [
    "--config",
    config,
    ...(host === undefined ? [] : ["--host", host]),
  ];
  run("docker", [
    ...docker,
    "pull",
    "--platform",
    "linux/arm64",
    settings.image,
  ]);
  const info = JSON.parse(
    run("docker", [...docker, "image", "inspect", settings.image], {
      encoding: "utf8",
      stdio: "pipe",
    }),
  )[0];
  if (info.Os !== "linux" || info.Architecture !== "arm64")
    throw new Error("linux/arm64 이미지 필요");
  writeFileSync(
    join(output, "image.json"),
    JSON.stringify(
      { tag: settings.image, id: info.Id, digests: info.RepoDigests },
      null,
      2,
    ),
  );
  run("pnpm", [
    "-C",
    "apps/desktop",
    "exec",
    "storybook",
    "build",
    "--output-dir",
    "../../target/DS-15/storybook",
  ]);
  run("docker", [
    ...docker,
    "run",
    "--rm",
    "--platform",
    "linux/arm64",
    "--network",
    "none",
    "--read-only",
    "--tmpfs",
    "/tmp",
    "--shm-size",
    "1g",
    "--mount",
    `type=bind,source=${root},target=/work,readonly`,
    "--mount",
    `type=bind,source=${output},target=/work/target/DS-15`,
    "--mount",
    `type=bind,source=${join(root, "harness/visual/baselines")},target=/work/harness/visual/baselines`,
    "--workdir",
    "/work",
    "--env",
    `VISUAL_IMAGE=${settings.image}`,
    info.Id,
    "node",
    "harness/visual/run.mjs",
    mode,
  ]);
} finally {
  rmSync(config, { recursive: true, force: true });
}
