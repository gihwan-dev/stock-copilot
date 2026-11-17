import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import {
  existsSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readlinkSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { homedir } from "node:os";
import { dirname, join, relative, resolve } from "node:path";
import { settings } from "../visual/policy.mjs";

const root = resolve(import.meta.dirname, "../..");
const output = join(root, "target/DS-20");

export const withBrowser = async (execute) => {
  mkdirSync(output, { recursive: true });
  const anonymous = mkdtempSync(join(output, "docker-anonymous-"));
  writeFileSync(join(anonymous, "config.json"), "{}\n");
  const run = (command, args, options = {}) => {
    const result = spawnSync(command, args, {
      cwd: root,
      encoding: "utf8",
      maxBuffer: 64 * 1024 * 1024,
      ...options,
    });
    if (result.status !== 0)
      throw new Error(
        `${command} 실패 (${result.status}):\n${result.stdout ?? ""}\n${result.stderr ?? ""}`,
      );
    return result.stdout ?? "";
  };
  const lock = readFileSync(join(root, "pnpm-lock.yaml"), "utf8");
  const binding =
    /'@rolldown\/binding-linux-arm64-gnu@([^']+)':\s*\n\s*resolution: \{integrity: sha512-([^}]+)\}/u.exec(
      lock,
    );
  if (binding === null) throw new Error("잠긴 Linux 바인딩이 없다");
  const [, version, integrity] = binding;
  const native = join(output, "linux-native");
  const archive = join(
    native,
    `rolldown-binding-linux-arm64-gnu-${version}.tgz`,
  );
  const extracted = join(
    native,
    "node_modules/@rolldown/binding-linux-arm64-gnu",
  );
  const link = join(
    root,
    `node_modules/.pnpm/rolldown@${version}/node_modules/@rolldown/binding-linux-arm64-gnu`,
  );
  const target = relative(dirname(link), extracted);
  let createdLink = false;

  try {
    mkdirSync(native, { recursive: true });
    if (!existsSync(archive)) {
      const response = await globalThis.fetch(
        `https://registry.npmjs.org/@rolldown/binding-linux-arm64-gnu/-/binding-linux-arm64-gnu-${version}.tgz`,
      );
      if (!response.ok)
        throw new Error(`잠긴 Linux 바인딩 다운로드 실패: ${response.status}`);
      writeFileSync(archive, Buffer.from(await response.arrayBuffer()));
    }
    const bytes = readFileSync(archive);
    if (createHash("sha512").update(bytes).digest("base64") !== integrity)
      throw new Error("잠긴 Linux 바인딩 무결성 불일치");
    mkdirSync(extracted, { recursive: true });
    run("tar", [
      "-xzf",
      archive,
      "--strip-components",
      "1",
      "-C",
      extracted,
      "package/package.json",
      "package/rolldown-binding.linux-arm64-gnu.node",
    ]);
    if (existsSync(link)) {
      const binary = "rolldown-binding.linux-arm64-gnu.node";
      const hash = (path) =>
        createHash("sha256")
          .update(readFileSync(join(path, binary)))
          .digest("hex");
      if (hash(link) !== hash(extracted))
        throw new Error("설치된 Linux 바인딩이 잠금과 다르다");
    } else {
      symlinkSync(target, link);
      createdLink = true;
    }
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
      anonymous,
      ...(host === undefined ? [] : ["--host", host]),
    ];
    const image = JSON.parse(
      run("docker", [...docker, "image", "inspect", settings.image]),
    )[0];
    const digest = settings.image.replace(/:[^/:@]+@/u, "@");
    if (
      image.Os !== "linux" ||
      image.Architecture !== "arm64" ||
      !image.RepoDigests.includes(digest)
    )
      throw new Error("DS-15 고정 이미지를 로컬에서 확인하지 못했다");
    const build = spawnSync(
      "pnpm",
      [
        "-C",
        "apps/desktop",
        "exec",
        "storybook",
        "build",
        "--disable-telemetry",
        "--output-dir",
        "../../target/DS-20/storybook",
      ],
      { cwd: root, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 },
    );
    if (build.status !== 0)
      throw new Error(`Storybook 빌드 실패:\n${build.stdout}\n${build.stderr}`);
    const index = JSON.parse(
      readFileSync(join(output, "storybook/index.json"), "utf8"),
    );
    const total = Object.values(index.entries).filter(
      (entry) => entry.type === "story",
    ).length;
    if (total === 0) throw new Error("측정할 이야기가 없다");
    const container = [
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
      `type=bind,source=${output},target=/work/target/DS-20`,
      "--workdir",
      "/work/apps/desktop",
      "--env",
      "CI=1",
      "--env",
      "XDG_DATA_HOME=/work/target/DS-20/vitest-data",
      "--env",
      "PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1",
      "--env",
      `VISUAL_IMAGE=${settings.image}`,
    ];
    const vitest = [
      "node",
      "node_modules/vitest/vitest.mjs",
      "run",
      "--config",
      "/work/harness/ds-overflow/browser-config.mjs",
      "--configLoader",
      "native",
      "--no-fsModuleCache",
      "--no-cache",
    ];
    return await execute({ root, output, container, vitest, image, total });
  } finally {
    if (
      createdLink &&
      existsSync(link) &&
      lstatSync(link).isSymbolicLink() &&
      readlinkSync(link) === target
    )
      rmSync(link);
    rmSync(anonymous, { recursive: true, force: true });
  }
};

export const runExamples = ({ root, output, container, vitest, image }) => {
  const reportPath = join(output, "probes-results.json");
  rmSync(reportPath, { force: true });
  const probes = spawnSync(
    "docker",
    [
      ...container,
      image.Id,
      ...vitest,
      "overflow.browser.ts",
      "--reporter=json",
      "--outputFile.json=/work/target/DS-20/probes-results.json",
    ],
    { cwd: root, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 },
  );
  writeFileSync(
    join(output, "probes.log"),
    `${probes.stdout ?? ""}${probes.stderr ?? ""}`,
  );
  if (probes.status !== 0 || !existsSync(reportPath))
    throw new Error("넘침 판정 예제 실패: target/DS-20/probes.log");
  const report = JSON.parse(readFileSync(reportPath, "utf8"));
  const assertions = report.testResults.flatMap(
    (suite) => suite.assertionResults,
  );
  if (
    !report.success ||
    report.numTotalTests < 79 ||
    report.numPendingTests !== 0 ||
    report.numTodoTests !== 0 ||
    assertions.length !== report.numTotalTests ||
    assertions.some((item) => item.status !== "passed")
  )
    throw new Error("넘침 판정 예제 실행이 불완전하다");
  process.stdout.write(`overflow: ${report.numTotalTests}개 판정 예제 통과\n`);
};
