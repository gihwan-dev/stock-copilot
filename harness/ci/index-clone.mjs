import { execFileSync } from "node:child_process";
import { resolve } from "node:path";

const cleanEnvironment = () => ({
  ...Object.fromEntries(
    Object.entries(process.env).filter(([name]) => !name.startsWith("GIT_")),
  ),
  GIT_CONFIG_GLOBAL: "/dev/null",
  GIT_CONFIG_NOSYSTEM: "1",
});

export const applyIndexToClone = (source, clone, indexFile) => {
  const environment = cleanEnvironment();
  const sourceEnvironment =
    indexFile === undefined
      ? environment
      : { ...environment, GIT_INDEX_FILE: resolve(source, indexFile) };
  const sourceTree = execFileSync("git", ["write-tree"], {
    cwd: source,
    env: sourceEnvironment,
    encoding: "utf8",
  }).trim();
  const patch = execFileSync(
    "git",
    [
      "diff",
      "--cached",
      "--binary",
      "--full-index",
      "--no-ext-diff",
      "--no-textconv",
      "HEAD",
      "--",
    ],
    { cwd: source, env: sourceEnvironment },
  );
  if (patch.length > 0) {
    execFileSync(
      "git",
      ["apply", "--index", "--binary", "--whitespace=nowarn", "-"],
      {
        cwd: clone,
        env: environment,
        input: patch,
      },
    );
  }
  const cloneTree = execFileSync("git", ["write-tree"], {
    cwd: clone,
    env: environment,
    encoding: "utf8",
  }).trim();
  if (cloneTree !== sourceTree) {
    throw new Error("복제본의 색인 트리가 원본과 다르다");
  }
  return patch.length > 0;
};

export const commitIndexSnapshot = (source, clone, indexFile) => {
  const changed = applyIndexToClone(source, clone, indexFile);
  if (changed) {
    execFileSync(
      "git",
      [
        "-c",
        "user.name=fixture",
        "-c",
        "user.email=fixture@example.invalid",
        "-c",
        "commit.gpgsign=false",
        "commit",
        "--quiet",
        "-m",
        "index snapshot",
      ],
      { cwd: clone, env: cleanEnvironment() },
    );
  }
  const head = execFileSync("git", ["rev-parse", "HEAD"], {
    cwd: clone,
    env: cleanEnvironment(),
    encoding: "utf8",
  }).trim();
  return { changed, head };
};

export const historicalMasterRef = (reference, originalHead, changed) =>
  changed ? reference.replace(/^master(?=[~^])/u, originalHead) : reference;
