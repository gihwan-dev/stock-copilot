import { spawnSync } from "node:child_process";
import {
  copyFileSync,
  cpSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

export const repoRoot = join(
  dirname(fileURLToPath(import.meta.url)),
  "..",
  "..",
);
export const listFile = join(
  repoRoot,
  "harness",
  "generated-files",
  "files.json",
);
export const desktopPrefix = "apps/desktop/";
export const recordedResponses = "apps/desktop/src-tauri/ipc/responses";
const prettierBin = join(repoRoot, "node_modules", ".bin", "prettier");

export const readList = () => JSON.parse(readFileSync(listFile, "utf8"));

export const generatedPaths = (list = readList()) =>
  list.files.map((entry) => entry.path);

export const desktopTypeScriptPaths = (list = readList()) =>
  generatedPaths(list)
    .filter((path) => path.startsWith(desktopPrefix) && path.endsWith(".ts"))
    .map((path) => path.slice(desktopPrefix.length));

const run = (command, args, cwd) => {
  const result = spawnSync(command, args, { cwd, encoding: "utf8" });
  return {
    ok: result.status === 0,
    output: `${result.stdout ?? ""}${result.stderr ?? ""}`,
  };
};

export const generateInto = (outRoot, list = readList()) => {
  rmSync(outRoot, { recursive: true, force: true });
  for (const path of generatedPaths(list)) {
    mkdirSync(dirname(join(outRoot, path)), { recursive: true });
  }
  cpSync(join(repoRoot, recordedResponses), join(outRoot, recordedResponses), {
    recursive: true,
  });
  const built = run(
    "cargo",
    [
      "build",
      "--locked",
      "--quiet",
      "--package",
      "src-tauri",
      "--bin",
      "gen-bindings",
    ],
    repoRoot,
  );
  if (!built.ok) return [`gen-bindings build failed: ${built.output}`];
  const generated = run(
    join(repoRoot, "target", "debug", "gen-bindings"),
    [],
    outRoot,
  );
  if (!generated.ok) return [`gen-bindings failed: ${generated.output}`];
  const formatted = list.files
    .filter((entry) => entry.format === "prettier")
    .map((entry) => join(outRoot, entry.path));
  const pretty = run(
    prettierBin,
    ["--write", "--log-level", "warn", ...formatted],
    repoRoot,
  );
  return pretty.ok ? [] : [`prettier failed: ${pretty.output}`];
};

export const compareGenerated = (outRoot, root = repoRoot, list = readList()) =>
  generatedPaths(list).flatMap((path) => {
    const committed = join(root, path);
    const fresh = join(outRoot, path);
    if (!existsSync(fresh)) return [`${path}: not generated`];
    if (!existsSync(committed))
      return [`${path}: missing; run just gen-bindings`];
    return readFileSync(committed, "utf8") === readFileSync(fresh, "utf8")
      ? []
      : [`${path}: differs from regenerated output; run just gen-bindings`];
  });

export const copyGenerated = (outRoot, root = repoRoot, list = readList()) => {
  for (const path of generatedPaths(list)) {
    copyFileSync(join(outRoot, path), join(root, path));
  }
};

export const scratchRoot = (name) =>
  join(mkdtempSync(join(tmpdir(), "generated-files-")), name);

export const relativeToRepo = (path) => relative(repoRoot, path);
