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
const archive = join(native, `rolldown-binding-linux-arm64-gnu-${version}.tgz`);
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
  const probes = spawnSync(
    "docker",
    [...container, image.Id, ...vitest, "overflow.browser.ts"],
    { cwd: root, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 },
  );
  writeFileSync(
    join(output, "probes.log"),
    `${probes.stdout ?? ""}${probes.stderr ?? ""}`,
  );
  if (probes.status !== 0)
    throw new Error("넘침 판정 예제 실패: target/DS-20/probes.log");
  process.stdout.write("overflow: 일곱 판정 예제 통과\n");
  const batches = [];
  for (let first = 0; first < total; first += 30) {
    const reportPath = join(output, `catalog-${first}.json`);
    rmSync(reportPath, { force: true });
    const result = spawnSync(
      "docker",
      [
        ...container,
        "--env",
        `DS20_FIRST=${first}`,
        "--env",
        "DS20_SIZE=30",
        image.Id,
        ...vitest,
        "catalog.browser.ts",
        "--reporter=default",
        "--reporter=json",
        `--outputFile.json=/work/target/DS-20/catalog-${first}.json`,
      ],
      { cwd: root, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 },
    );
    const log = `${result.stdout ?? ""}${result.stderr ?? ""}`.replace(
      new RegExp(`${String.fromCharCode(27)}\\[[0-9;]*m`, "gu"),
      "",
    );
    writeFileSync(join(output, `catalog-${first}.log`), log);
    const line = log
      .split("\n")
      .find((item) => item.includes("DS20_CATALOG_FINDINGS "));
    if (line === undefined)
      throw new Error(
        `이야기 측정 누락: ${first}, target/DS-20/catalog-${first}.log`,
      );
    const value = JSON.parse(
      line.slice(
        line.indexOf("DS20_CATALOG_FINDINGS ") +
          "DS20_CATALOG_FINDINGS ".length,
      ),
    );
    if (
      value.first !== first ||
      value.total !== total ||
      value.count !== Math.min(30, total - first) ||
      value.errors.length !== 0
    )
      throw new Error(`이야기 측정 범위 불일치: ${first}`);
    if (
      !existsSync(reportPath) ||
      log.includes("Unhandled Error") ||
      log.includes("Failed Suites") ||
      log.includes("Vite unexpectedly reloaded")
    )
      throw new Error(`이야기 실행 자체가 불완전하다: ${first}`);
    const report = JSON.parse(readFileSync(reportPath, "utf8"));
    const assertions = report.testResults.flatMap(
      (suite) => suite.assertionResults,
    );
    const expectedStatus = value.results.length === 0 ? "passed" : "failed";
    if (
      report.numTotalTests !== 3 ||
      report.numPendingTests !== 0 ||
      report.numTodoTests !== 0 ||
      assertions.length !== 3 ||
      assertions[0].status !== "passed" ||
      assertions[1].status !== "passed" ||
      assertions[2].status !== expectedStatus
    )
      throw new Error(`이야기 테스트 실행 결과 불완전: ${first}`);
    if (result.status !== (value.results.length === 0 ? 0 : 1))
      throw new Error(`이야기 측정 종료 상태 불일치: ${first}`);
    batches.push(value);
    process.stdout.write(
      `overflow: ${first + value.count}/${total}, 위반 조건 ${value.results.length}\n`,
    );
  }
  const findings = batches.flatMap((batch) => batch.results);
  writeFileSync(
    join(output, "catalog-findings.json"),
    `${JSON.stringify({ total, batches: batches.length, findings }, null, 2)}\n`,
  );
  process.stdout.write(
    `overflow: ${total} 이야기, 위반 조건 ${findings.length}\n`,
  );
  if (findings.length !== 0) process.exitCode = 1;
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
