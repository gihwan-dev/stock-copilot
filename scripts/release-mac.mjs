#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
export const SIGNING_IDENTITY =
  "Developer ID Application: GIHWAN CHOI (SLVS4WF9U2)";
export const KEYCHAIN_SERVICE = "dev.stockcopilot.updater-key";
export const KEY_ACCOUNT = "private-key";
export const PASSWORD_ACCOUNT = "password";
export const PUBKEY_PLACEHOLDER = "STOCK_COPILOT_UPDATER_PUBKEY_PLACEHOLDER";
export const REPOSITORY = "gihwan-dev/stock-copilot";
export const ASSET_NAME = "Stock.Copilot.app.tar.gz";
export const ENDPOINT = `https://github.com/${REPOSITORY}/releases/latest/download/latest.json`;

const CARGO_TOML = "Cargo.toml";
const TAURI_CONF = "apps/desktop/src-tauri/tauri.conf.json";
const ENTITLEMENTS = "apps/desktop/src-tauri/Entitlements.plist";
const BUNDLE_DIR = "target/release/bundle/macos";
const APP_NAME = "Stock Copilot.app";

export const paths = {
  bundleDir: BUNDLE_DIR,
  app: `${BUNDLE_DIR}/${APP_NAME}`,
  archive: `${BUNDLE_DIR}/${ASSET_NAME}`,
  signature: `${BUNDLE_DIR}/${ASSET_NAME}.sig`,
  latest: `${BUNDLE_DIR}/latest.json`,
  notes: `${BUNDLE_DIR}/release-notes.md`,
  bridge: "target/release/mcp-bridge",
};

export class ReleaseError extends Error {}

export const parseArgs = (argv) => {
  const flags = new Set(argv.filter((item) => item.startsWith("--")));
  const words = argv.filter((item) => !item.startsWith("--"));
  const notesAt = argv.indexOf("--notes");
  const notesFile = notesAt === -1 ? undefined : argv[notesAt + 1];
  const positional = words.filter((item) => item !== notesFile);
  const command = flags.has("--init-key") ? "init-key" : positional[0];
  const version = command === "release" ? positional[1] : undefined;
  return {
    command,
    version,
    notesFile,
    dryRun: flags.has("--dry-run") ? "dry-run" : "run",
  };
};

export const validateVersion = (version) => {
  if (typeof version !== "string" || !/^\d+\.\d+\.\d+$/u.test(version)) {
    throw new ReleaseError(
      `버전은 1.2.3 형식이어야 해요: ${version ?? "(없음)"}`,
    );
  }
  return version;
};

export const bumpCargoToml = (text, version) => {
  const section = /(\[workspace\.package\][^[]*?\nversion = ")([^"]+)(")/u;
  if (!section.test(text)) {
    throw new ReleaseError(
      "Cargo.toml에서 [workspace.package] version을 찾지 못했어요",
    );
  }
  return text.replace(section, `$1${version}$3`);
};

export const currentVersion = (text) => {
  const found = /\[workspace\.package\][^[]*?\nversion = "([^"]+)"/u.exec(text);
  if (found === null) {
    throw new ReleaseError("Cargo.toml에서 현재 버전을 찾지 못했어요");
  }
  return found[1];
};

export const writePubkey = (confText, pubkey) => {
  const conf = JSON.parse(confText);
  const updater = conf.plugins?.updater;
  if (updater === undefined) {
    throw new ReleaseError("tauri.conf.json에 plugins.updater가 없어요");
  }
  if (updater.pubkey !== PUBKEY_PLACEHOLDER) {
    throw new ReleaseError(
      "tauri.conf.json에 이미 업데이트 공개키가 있어요. 키를 바꾸려면 손으로 지우고 다시 실행하세요",
    );
  }
  const key = pubkey.trim();
  if (key === "" || /\s/u.test(key)) {
    throw new ReleaseError("공개키 형식이 이상해요");
  }
  return confText.replace(
    `"pubkey": "${PUBKEY_PLACEHOLDER}"`,
    `"pubkey": "${key}"`,
  );
};

export const platformKey = (arch) => {
  switch (arch) {
    case "arm64":
      return "darwin-aarch64";
    case "x64":
      return "darwin-x86_64";
    default:
      throw new ReleaseError(`지원하지 않는 아키텍처예요: ${arch}`);
  }
};

export const assetUrl = (version) =>
  `https://github.com/${REPOSITORY}/releases/download/v${version}/${ASSET_NAME}`;

export const latestJson = ({ version, notes, pubDate, signature, arch }) =>
  `${JSON.stringify(
    {
      version,
      notes,
      pub_date: pubDate,
      platforms: {
        [platformKey(arch)]: {
          signature: signature.trim(),
          url: assetUrl(version),
        },
      },
    },
    null,
    2,
  )}\n`;

const command = (title, program, args, options = {}) => ({
  title,
  kind: "command",
  program,
  args,
  env: options.env ?? {},
  secretEnv: options.secretEnv ?? [],
  input: options.input,
  capture: options.capture ?? "inherit",
});

const action = (title, describe, run) => ({
  title,
  kind: "action",
  describe,
  run,
});

const quote = (value) =>
  /^[\w@%+=:,./-]+$/u.test(value) ? value : `"${value.replaceAll('"', '\\"')}"`;

export const describeStep = (step) => {
  if (step.kind === "action") return step.describe;
  const env = Object.entries(step.env).map(
    ([name, value]) => `${name}=${quote(value)}`,
  );
  const secrets = step.secretEnv.map((name) => `${name}=<키체인>`);
  return [...env, ...secrets, step.program, ...step.args.map(quote)].join(" ");
};

export const releasePlan = ({ version, notesFile, root = repoRoot }) => {
  const at = (relative) => join(root, relative);
  return [
    command("작업 사본 확인", "git", ["status", "--porcelain"], {
      capture: "require-empty",
    }),
    action(
      "버전 올리기",
      `${CARGO_TOML}의 [workspace.package] version = "${version}"`,
      () => {
        const file = at(CARGO_TOML);
        writeFileSync(file, bumpCargoToml(readFileSync(file, "utf8"), version));
      },
    ),
    command("잠금 파일 갱신", "cargo", ["update", "--workspace", "--offline"]),
    command("버전 커밋", "git", ["commit", "-am", `release: v${version}`]),
    command("태그", "git", ["tag", `v${version}`]),
    command(
      "앱 빌드",
      "pnpm",
      ["-C", "apps/desktop", "exec", "tauri", "build", "--bundles", "app"],
      { env: { APPLE_SIGNING_IDENTITY: SIGNING_IDENTITY } },
    ),
    command("브리지 빌드", "cargo", [
      "build",
      "--release",
      "--locked",
      "-p",
      "mcp-bridge",
    ]),
    command("생성 도구 빼기", "rm", [
      "-f",
      `${paths.app}/Contents/MacOS/gen-bindings`,
    ]),
    command("브리지 넣기", "cp", [
      paths.bridge,
      `${paths.app}/Contents/MacOS/mcp-bridge`,
    ]),
    command("브리지 서명", "codesign", [
      "--force",
      "--options",
      "runtime",
      "--timestamp",
      "--sign",
      SIGNING_IDENTITY,
      `${paths.app}/Contents/MacOS/mcp-bridge`,
    ]),
    command("앱 서명", "codesign", [
      "--force",
      "--options",
      "runtime",
      "--timestamp",
      "--entitlements",
      ENTITLEMENTS,
      "--sign",
      SIGNING_IDENTITY,
      paths.app,
    ]),
    command("서명 확인", "codesign", [
      "--verify",
      "--deep",
      "--strict",
      paths.app,
    ]),
    command("업데이트 묶음", "tar", [
      "-czf",
      paths.archive,
      "-C",
      paths.bundleDir,
      APP_NAME,
    ]),
    command(
      "업데이트 서명",
      "pnpm",
      [
        "-C",
        "apps/desktop",
        "exec",
        "tauri",
        "signer",
        "sign",
        at(paths.archive),
      ],
      {
        secretEnv: [
          "TAURI_SIGNING_PRIVATE_KEY",
          "TAURI_SIGNING_PRIVATE_KEY_PASSWORD",
        ],
      },
    ),
    action(
      "릴리스 노트",
      `${notesFile ?? "직전 태그 이후 커밋 제목"} → ${paths.notes}`,
      () => {
        writeFileSync(
          at(paths.notes),
          releaseNotes({ version, notesFile, root }),
        );
      },
    ),
    action("latest.json", `${paths.latest} (${ENDPOINT})`, () => {
      writeFileSync(
        at(paths.latest),
        latestJson({
          version,
          notes: readFileSync(at(paths.notes), "utf8").trim(),
          pubDate: new Date().toISOString(),
          signature: readFileSync(at(paths.signature), "utf8"),
          arch: process.arch,
        }),
      );
    }),
    command("태그 올리기", "git", ["push", "origin", `refs/tags/v${version}`]),
    command("GitHub 릴리스", "gh", [
      "release",
      "create",
      `v${version}`,
      "--verify-tag",
      paths.archive,
      paths.latest,
      "--repo",
      REPOSITORY,
      "--title",
      `Stock Copilot ${version}`,
      "--notes-file",
      paths.notes,
    ]),
  ];
};

export const initKeyPlan = ({ root = repoRoot } = {}) => [
  command(
    "기존 키 확인",
    "security",
    ["find-generic-password", "-s", KEYCHAIN_SERVICE, "-a", KEY_ACCOUNT],
    { capture: "require-missing" },
  ),
  action(
    "키 만들기",
    "임시 0700 폴더에서 pnpm -C apps/desktop exec tauri signer generate --ci -w <임시>/updater.key",
    (state) => {
      state.folder = mkdtempSync(join(tmpdir(), "stock-copilot-updater-"));
      state.keyFile = join(state.folder, "updater.key");
      run(
        command("키 만들기", "pnpm", [
          "-C",
          "apps/desktop",
          "exec",
          "tauri",
          "signer",
          "generate",
          "--ci",
          "-w",
          state.keyFile,
        ]),
        root,
      );
    },
  ),
  action(
    "키체인에 저장",
    `security -i 표준 입력으로 add-generic-password -s ${KEYCHAIN_SERVICE} -a ${KEY_ACCOUNT} (비밀키는 인자에 넣지 않는다)`,
    (state) => {
      const secret = readFileSync(state.keyFile, "utf8").trim();
      if (!/^[A-Za-z0-9+/=]+$/u.test(secret)) {
        throw new ReleaseError("만든 비밀키 형식이 이상해요");
      }
      run(
        command("키체인에 저장", "security", ["-i"], {
          input: `add-generic-password -U -s ${KEYCHAIN_SERVICE} -a ${KEY_ACCOUNT} -w "${secret}"\n`,
        }),
        root,
      );
    },
  ),
  action(
    "공개키 넣기",
    `${TAURI_CONF}의 plugins.updater.pubkey 자리표시를 공개키로 바꾼다`,
    (state) => {
      const file = join(root, TAURI_CONF);
      const pubkey = readFileSync(`${state.keyFile}.pub`, "utf8");
      writeFileSync(file, writePubkey(readFileSync(file, "utf8"), pubkey));
    },
  ),
  action("임시 폴더 지우기", "임시 폴더를 지운다", (state) => {
    rmSync(state.folder, { recursive: true, force: true });
  }),
];

export const releaseNotes = ({ version, notesFile, root = repoRoot }) => {
  if (notesFile !== undefined) return readFileSync(resolve(notesFile), "utf8");
  const previous = spawnSync(
    "git",
    ["describe", "--tags", "--abbrev=0", `v${version}^`],
    {
      cwd: root,
      encoding: "utf8",
    },
  );
  const range =
    previous.status === 0
      ? `${previous.stdout.trim()}..v${version}`
      : `v${version}`;
  const log = spawnSync(
    "git",
    ["log", "--no-merges", "--pretty=format:- %s", range],
    { cwd: root, encoding: "utf8" },
  );
  const lines = log.status === 0 ? log.stdout.trim() : "";
  return lines === "" ? `- v${version}\n` : `${lines}\n`;
};

const keychainValue = (account, required) => {
  const found = spawnSync(
    "security",
    ["find-generic-password", "-s", KEYCHAIN_SERVICE, "-a", account, "-w"],
    { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] },
  );
  if (found.status === 0) return found.stdout.trim();
  if (required === "required") {
    throw new ReleaseError(
      `키체인에 ${KEYCHAIN_SERVICE}/${account} 항목이 없어요. 먼저 init-key를 실행하세요`,
    );
  }
  return "";
};

const secretValues = {
  TAURI_SIGNING_PRIVATE_KEY: () => keychainValue(KEY_ACCOUNT, "required"),
  TAURI_SIGNING_PRIVATE_KEY_PASSWORD: () =>
    keychainValue(PASSWORD_ACCOUNT, "optional"),
};

const run = (step, root) => {
  const env = {
    ...process.env,
    ...step.env,
    ...Object.fromEntries(
      step.secretEnv.map((name) => [name, secretValues[name]()]),
    ),
  };
  const stdio =
    step.input !== undefined
      ? ["pipe", "ignore", "inherit"]
      : step.capture === "inherit"
        ? "inherit"
        : ["ignore", "pipe", "pipe"];
  const result = spawnSync(step.program, step.args, {
    cwd: root,
    env,
    input: step.input,
    stdio,
    encoding: "utf8",
  });
  if (step.capture === "require-missing") {
    if (result.status === 0) {
      throw new ReleaseError(
        `키체인에 이미 ${KEYCHAIN_SERVICE} 항목이 있어요. 덮어쓰지 않아요`,
      );
    }
    return;
  }
  if (result.status !== 0) {
    throw new ReleaseError(
      `${step.title} 단계가 실패했어요 (종료 ${result.status})`,
    );
  }
  if (step.capture === "require-empty" && result.stdout.trim() !== "") {
    throw new ReleaseError("작업 사본에 커밋하지 않은 변경이 있어요");
  }
};

export const execute = (
  plan,
  { dryRun, root = repoRoot, write = (line) => process.stdout.write(line) },
) => {
  const state = {};
  plan.forEach((step, index) => {
    write(
      `[${index + 1}/${plan.length}] ${step.title}: ${describeStep(step)}\n`,
    );
    if (dryRun === "dry-run") return;
    if (step.kind === "action") step.run(state);
    else run(step, root);
  });
};

const usage = `사용법:
  node scripts/release-mac.mjs release <버전> [--notes <파일>] [--dry-run]
  node scripts/release-mac.mjs init-key [--dry-run]   (또는 --init-key)
`;

export const main = (argv) => {
  const options = parseArgs(argv);
  switch (options.command) {
    case "release": {
      const version = validateVersion(options.version);
      const from = currentVersion(
        readFileSync(join(repoRoot, CARGO_TOML), "utf8"),
      );
      process.stdout.write(`Stock Copilot ${from} → ${version}\n`);
      execute(releasePlan({ version, notesFile: options.notesFile }), options);
      if (options.dryRun === "run") {
        process.stdout.write(
          `끝났어요. 커밋과 태그를 올리세요: git push origin HEAD v${version}\n`,
        );
      }
      return;
    }
    case "init-key":
      execute(initKeyPlan(), options);
      if (options.dryRun === "run") {
        process.stdout.write(
          `공개키를 ${TAURI_CONF}에 넣었어요. 커밋하세요. 비밀키는 키체인(${KEYCHAIN_SERVICE})에만 있어요\n`,
        );
      }
      return;
    default:
      throw new ReleaseError(usage);
  }
};

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    main(process.argv.slice(2));
  } catch (error) {
    process.stderr.write(
      `${error instanceof ReleaseError ? error.message : error}\n`,
    );
    process.exitCode = 1;
  }
}
