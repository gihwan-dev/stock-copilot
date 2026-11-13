import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  chmodSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const TOOLS_FILE = "harness/ci/tools.json";

const LEFTHOOK_RELEASES =
  "https://github.com/evilmartians/lefthook/releases/download";

export const readTools = (root) =>
  JSON.parse(readFileSync(join(root, TOOLS_FILE), "utf8"));

export const installActionSpec = (tools) =>
  Object.entries(tools.installAction.tools)
    .map(([name, version]) => `${name}@${version}`)
    .join(",");

export const platformKey = (os, arch) => `${os}-${arch}`;

export const lefthookAsset = (tools, key) => {
  const asset = tools.lefthook.assets[key];
  return asset === undefined
    ? { problem: `lefthook: ${key}용 고정 파일이 없다 (${TOOLS_FILE})` }
    : {
        url: `${LEFTHOOK_RELEASES}/v${tools.lefthook.version}/${asset.file}`,
        sha256: asset.sha256,
      };
};

export const sha256 = (bytes) =>
  createHash("sha256").update(bytes).digest("hex");

export const prefetchDependencies = (source) =>
  [
    ...source.matchAll(
      /([A-Za-z0-9_-]+) = \{ version = \\"=([0-9][^\\"]*)\\", default-features = false \}/gu,
    ),
  ].map((match) => ({ name: match[1], version: match[2] }));

export const prefetchManifest = (dependencies) =>
  [
    "[package]",
    'name = "harness-prefetch"',
    'version = "0.0.0"',
    'edition = "2024"',
    "",
    "[dependencies]",
    ...dependencies.map(
      ({ name, version }) =>
        `${name} = { version = "=${version}", default-features = false }`,
    ),
    "",
  ].join("\n");

const run = (command, args, options = {}) => {
  const finished = spawnSync(command, args, { stdio: "inherit", ...options });
  return finished.status === 0
    ? 0
    : (process.stderr.write(`${command} ${args.join(" ")}: 실패\n`), 1);
};

const installLefthook = (root, tools, directory) => {
  const asset = lefthookAsset(
    tools,
    platformKey(process.env.RUNNER_OS ?? "", process.env.RUNNER_ARCH ?? ""),
  );
  if (asset.problem !== undefined) {
    process.stderr.write(`${asset.problem}\n`);
    return 1;
  }
  const download = spawnSync("curl", ["-sSfL", asset.url], {
    maxBuffer: 256 * 1024 * 1024,
  });
  if (download.status !== 0) {
    process.stderr.write(`lefthook 내려받기 실패: ${asset.url}\n`);
    return 1;
  }
  const actual = sha256(download.stdout);
  if (actual !== asset.sha256) {
    process.stderr.write(
      `lefthook 해시가 다르다: 기대 ${asset.sha256}, 실제 ${actual}\n`,
    );
    return 1;
  }
  mkdirSync(directory, { recursive: true });
  const target = join(directory, "lefthook");
  writeFileSync(target, download.stdout);
  chmodSync(target, 0o755);
  process.stdout.write(`lefthook ${tools.lefthook.version}: ${target}\n`);
  return 0;
};

const installCrates = (root, tools) =>
  Object.entries(tools.cargoInstall).reduce(
    (status, [name, version]) =>
      status !== 0
        ? status
        : run("cargo", ["install", "--locked", "--version", version, name], {
            cwd: root,
          }),
    0,
  );

const prefetch = (root, tools) => {
  const dependencies = prefetchDependencies(
    readFileSync(join(root, tools.prefetch.source), "utf8"),
  );
  if (dependencies.length === 0) {
    process.stderr.write(
      `prefetch: ${tools.prefetch.source}에 의존성이 없다\n`,
    );
    return 1;
  }
  const directory = mkdtempSync(join(tmpdir(), "harness-prefetch-"));
  try {
    mkdirSync(join(directory, "src"));
    writeFileSync(join(directory, "src", "lib.rs"), "");
    writeFileSync(
      join(directory, "Cargo.toml"),
      prefetchManifest(dependencies),
    );
    return run(
      "cargo",
      ["fetch", "--manifest-path", join(directory, "Cargo.toml")],
      { cwd: root },
    );
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
};

const repoRoot = resolve(fileURLToPath(import.meta.url), "..", "..", "..");

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const tools = readTools(repoRoot);
  const [command, argument] = process.argv.slice(2);
  process.exitCode =
    command === "install-action"
      ? (process.stdout.write(`${installActionSpec(tools)}\n`), 0)
      : command === "lefthook" && argument !== undefined
        ? installLefthook(repoRoot, tools, resolve(argument))
        : command === "cargo-install"
          ? installCrates(repoRoot, tools)
          : command === "prefetch"
            ? prefetch(repoRoot, tools)
            : (process.stderr.write(
                "사용법: node harness/ci/tools.mjs install-action | lefthook <폴더> | cargo-install | prefetch\n",
              ),
              2);
}
