import { spawnSync } from "node:child_process";
import { createHash, randomBytes } from "node:crypto";
import {
  chmodSync,
  constants,
  copyFileSync,
  cpSync,
  existsSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  realpathSync,
  renameSync,
  rmSync,
  rmdirSync,
  statSync,
  utimesSync,
  writeFileSync,
} from "node:fs";
import { homedir } from "node:os";
import {
  basename,
  dirname,
  isAbsolute,
  join,
  relative,
  resolve,
  sep,
} from "node:path";
import { fileURLToPath } from "node:url";

const MODULE_DIR = dirname(fileURLToPath(import.meta.url));
export const POLICY_PATH = join(MODULE_DIR, "policy.json");
const OUT_DIR_TOKEN = "__H68_OUT_DIR__";
const BUILD_OUT_DIR_TOKEN = "__H68_BUILD_OUT_DIR__";

const EXACT_ENV_KEYS = [
  "TARGET",
  "HOST",
  "OPT_LEVEL",
  "DEBUG",
  "PROFILE",
  "RUSTFLAGS",
  "CARGO_ENCODED_RUSTFLAGS",
  "RUSTC_BOOTSTRAP",
  "CARGO_CRATE_NAME",
  "CARGO_BIN_NAME",
];

const ENV_PREFIXES = ["CARGO_PKG_", "CARGO_CFG_", "CARGO_FEATURE_", "DEP_"];

const UNSUPPORTED_FLAGS = new Set([
  "-vV",
  "--version",
  "-V",
  "--print",
  "--explain",
  "-h",
  "--help",
  "-o",
]);

const SYSTEM_PREFIXES = [
  "/System/",
  "/Library/",
  "/Applications/",
  "/usr/",
  "/opt/homebrew/",
];

export const readPolicy = (policyPath = POLICY_PATH) =>
  JSON.parse(readFileSync(policyPath, "utf8"));

export const resolveCargoHome = (env = process.env) =>
  resolve(env.CARGO_HOME || join(homedir(), ".cargo"));

export const resolveCacheDir = (env = process.env, policy = readPolicy()) => {
  if (
    typeof env.STOCK_COPILOT_BUILD_CACHE_DIR === "string" &&
    env.STOCK_COPILOT_BUILD_CACHE_DIR.length > 0
  ) {
    return resolve(env.STOCK_COPILOT_BUILD_CACHE_DIR);
  }
  return join(resolveCargoHome(env), policy.cacheSubdir);
};

export const isSafeArtifactName = (name) =>
  typeof name === "string" &&
  name.length > 0 &&
  name !== "." &&
  name !== ".." &&
  !name.includes("/") &&
  !name.includes("\\") &&
  !name.includes("\0");

const ignoreBestEffortError = () => {};

const sha256Buffer = (buffer) =>
  createHash("sha256").update(buffer).digest("hex");

const sha256File = (filePath) => sha256Buffer(readFileSync(filePath));

const isInside = (parent, candidate) =>
  candidate === parent || candidate.startsWith(`${parent}${sep}`);

const safeRealpath = (targetPath) => {
  try {
    return realpathSync(targetPath);
  } catch {
    return undefined;
  }
};

export const hashDirectoryBounded = (rootDir, maxBytes, maxFiles) => {
  if (!existsSync(rootDir)) {
    return { ok: true, hash: sha256Buffer(Buffer.from("missing")) };
  }
  const rootStat = lstatSync(rootDir);
  if (rootStat.isSymbolicLink() || !rootStat.isDirectory()) {
    return { ok: false };
  }
  const files = [];
  let totalBytes = 0;
  const walk = (current) => {
    const entries = readdirSync(current, { withFileTypes: true }).sort(
      (left, right) => left.name.localeCompare(right.name),
    );
    for (const entry of entries) {
      const fullPath = join(current, entry.name);
      const st = lstatSync(fullPath);
      if (st.isSymbolicLink()) {
        return false;
      }
      if (st.isDirectory()) {
        if (!walk(fullPath)) {
          return false;
        }
      } else if (st.isFile()) {
        totalBytes += st.size;
        if (files.length >= maxFiles || totalBytes > maxBytes) {
          return false;
        }
        files.push({
          rel: relative(rootDir, fullPath).split(sep).join("/"),
          mode: st.mode & 0o777,
          sha256: sha256File(fullPath),
        });
      } else {
        return false;
      }
    }
    return true;
  };
  if (!walk(rootDir)) {
    return { ok: false };
  }
  return {
    ok: true,
    hash: sha256Buffer(Buffer.from(JSON.stringify(files))),
  };
};

const replaceTokens = (text, replacements, token) => {
  let result = text;
  for (const needle of replacements) {
    if (needle.length > 0) {
      result = result.split(needle).join(token);
    }
  }
  return result;
};

const extractProfileAndWorktree = (outDir) => {
  const resolvedOut = resolve(outDir);
  const outBase = basename(resolvedOut);
  const outParent = dirname(resolvedOut);
  let profileDir;
  if (outBase === "deps") {
    profileDir = outParent;
  } else if (basename(outParent) === "build") {
    profileDir = dirname(outParent);
  } else {
    return undefined;
  }
  const marker = `${sep}target${sep}`;
  const markerIndex = profileDir.lastIndexOf(marker);
  const worktreeRoot = markerIndex > 0 ? profileDir.slice(0, markerIndex) : "";
  return { outDir: resolvedOut, profileDir, worktreeRoot };
};

const isCFlagBlocked = (value) =>
  value.startsWith("incremental=") ||
  value === "instrument-coverage" ||
  value.startsWith("instrument-coverage=") ||
  value.startsWith("profile-generate") ||
  value.startsWith("profile-use");

const splitSearchPath = (raw) => {
  const match = /^(dependency|crate|native|framework|all)=(.+)$/u.exec(raw);
  if (match === null) {
    return { kind: "", dir: raw };
  }
  return { kind: match[1], dir: match[2] };
};

const hashExternValue = (raw, cwd, worktreeRoot, profileDir) => {
  const eq = raw.indexOf("=");
  if (eq === -1) {
    return { ok: true, value: raw };
  }
  const prefix = raw.slice(0, eq);
  const rawPath = raw.slice(eq + 1);
  const fullPath = isAbsolute(rawPath)
    ? resolve(rawPath)
    : resolve(cwd, rawPath);
  if (
    worktreeRoot !== "" &&
    isInside(worktreeRoot, fullPath) &&
    !isInside(profileDir, fullPath)
  ) {
    return { ok: false };
  }
  try {
    const st = lstatSync(fullPath);
    if (!st.isFile() && !st.isSymbolicLink()) {
      return { ok: false };
    }
    return { ok: true, value: `${prefix}=<SHA256:${sha256File(fullPath)}>` };
  } catch {
    return { ok: false };
  }
};

const canonicalizeSearchPath = (
  raw,
  cwd,
  worktreeRoot,
  profileDir,
  registrySrcReal,
  policy,
) => {
  const { kind, dir } = splitSearchPath(raw);
  const fullDir = isAbsolute(dir) ? resolve(dir) : resolve(cwd, dir);
  const realDir = safeRealpath(fullDir) ?? fullDir;
  const tag = kind === "" ? "" : `${kind}=`;
  if (isInside(profileDir, fullDir)) {
    const rel = relative(profileDir, fullDir).split(sep).join("/");
    if (kind === "dependency" || kind === "crate") {
      return { ok: true, value: `${tag}<PROFILE_DIR>/${rel}` };
    }
    const hashed = hashDirectoryBounded(fullDir, policy.maxNativeDirBytes, 256);
    if (!hashed.ok) {
      return { ok: false };
    }
    return { ok: true, value: `${tag}<NATIVE_DIR:${hashed.hash}>` };
  }
  if (isInside(registrySrcReal, realDir)) {
    const rel = relative(registrySrcReal, realDir).split(sep).join("/");
    return { ok: true, value: `${tag}<REGISTRY_SRC>/${rel}` };
  }
  if (worktreeRoot !== "" && isInside(worktreeRoot, fullDir)) {
    return { ok: false };
  }
  if (SYSTEM_PREFIXES.some((prefix) => fullDir.startsWith(prefix))) {
    return { ok: true, value: `${tag}${fullDir}` };
  }
  return { ok: false };
};

export const parseInvocation = ({
  rustc,
  args,
  env = process.env,
  cwd = process.cwd(),
  policy = readPolicy(),
}) => {
  if (
    (env.CI ?? "") !== "" ||
    (env.GITHUB_ACTIONS ?? "") !== "" ||
    env.STOCK_COPILOT_BUILD_CACHE === "off" ||
    (env.CARGO_PRIMARY_PACKAGE ?? "") !== "" ||
    (env.LLVM_PROFILE_FILE ?? "") !== ""
  ) {
    return undefined;
  }
  if (basename(rustc) !== "rustc") {
    return undefined;
  }
  const cargoHome = resolveCargoHome(env);
  const registrySrcReal = safeRealpath(join(cargoHome, "registry", "src"));
  if (registrySrcReal === undefined) {
    return undefined;
  }
  let outDir;
  let outDirArgIndex = -1;
  let outDirInline = false;
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (
      UNSUPPORTED_FLAGS.has(arg) ||
      arg.startsWith("--print=") ||
      arg.startsWith("@") ||
      arg.startsWith("-o")
    ) {
      return undefined;
    }
    if (arg === "-C" && index + 1 < args.length) {
      if (isCFlagBlocked(args[index + 1])) {
        return undefined;
      }
      index += 1;
      continue;
    }
    if (arg.startsWith("-C") && isCFlagBlocked(arg.slice(2))) {
      return undefined;
    }
    if (arg === "--out-dir" && index + 1 < args.length) {
      if (outDir !== undefined) {
        return undefined;
      }
      outDir = args[index + 1];
      outDirArgIndex = index;
      outDirInline = false;
      index += 1;
      continue;
    }
    if (arg.startsWith("--out-dir=")) {
      if (outDir !== undefined) {
        return undefined;
      }
      outDir = arg.slice("--out-dir=".length);
      outDirArgIndex = index;
      outDirInline = true;
    }
  }
  if (outDir === undefined) {
    return undefined;
  }
  const layout = extractProfileAndWorktree(outDir);
  if (layout === undefined) {
    return undefined;
  }
  const { profileDir, worktreeRoot } = layout;
  const canonicalArgs = [];
  let sourceReal;
  let sourceRel;
  let sourceSha256;
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (index === outDirArgIndex) {
      if (outDirInline) {
        canonicalArgs.push("--out-dir=<OUT_DIR>");
      } else {
        canonicalArgs.push("--out-dir", "<OUT_DIR>");
        index += 1;
      }
      continue;
    }
    if (arg === "--extern" && index + 1 < args.length) {
      const hashed = hashExternValue(
        args[index + 1],
        cwd,
        worktreeRoot,
        profileDir,
      );
      if (!hashed.ok) {
        return undefined;
      }
      canonicalArgs.push("--extern", hashed.value);
      index += 1;
      continue;
    }
    if (arg.startsWith("--extern=")) {
      const hashed = hashExternValue(
        arg.slice("--extern=".length),
        cwd,
        worktreeRoot,
        profileDir,
      );
      if (!hashed.ok) {
        return undefined;
      }
      canonicalArgs.push(`--extern=${hashed.value}`);
      continue;
    }
    if (arg === "-L" && index + 1 < args.length) {
      const mapped = canonicalizeSearchPath(
        args[index + 1],
        cwd,
        worktreeRoot,
        profileDir,
        registrySrcReal,
        policy,
      );
      if (!mapped.ok) {
        return undefined;
      }
      canonicalArgs.push("-L", mapped.value);
      index += 1;
      continue;
    }
    if (arg.startsWith("-L=") || (arg.startsWith("-L") && arg.length > 2)) {
      const raw = arg.startsWith("-L=") ? arg.slice(3) : arg.slice(2);
      const mapped = canonicalizeSearchPath(
        raw,
        cwd,
        worktreeRoot,
        profileDir,
        registrySrcReal,
        policy,
      );
      if (!mapped.ok) {
        return undefined;
      }
      canonicalArgs.push(`-L${mapped.value}`);
      continue;
    }
    if (arg.startsWith("--remap-path-prefix")) {
      return undefined;
    }
    if (!arg.startsWith("-") && arg.endsWith(".rs")) {
      const candidatePath = isAbsolute(arg) ? arg : resolve(cwd, arg);
      const candidateReal = safeRealpath(candidatePath);
      if (
        candidateReal === undefined ||
        !isInside(registrySrcReal, candidateReal) ||
        sourceReal !== undefined
      ) {
        return undefined;
      }
      sourceReal = candidateReal;
      sourceRel = relative(registrySrcReal, candidateReal).split(sep).join("/");
      sourceSha256 = sha256File(candidateReal);
      canonicalArgs.push(`<REGISTRY_SRC>/${sourceRel}`);
      continue;
    }
    if (
      arg.includes(profileDir) ||
      (worktreeRoot !== "" && arg.includes(worktreeRoot))
    ) {
      return undefined;
    }
    canonicalArgs.push(arg);
  }
  if (sourceReal === undefined) {
    return undefined;
  }
  const canonicalEnv = {};
  const hashedEnvNames = new Set(["OUT_DIR"]);
  const sortedKeys = Object.keys(env).sort();
  for (const key of sortedKeys) {
    const value = env[key];
    if (value === undefined) {
      continue;
    }
    if (key === "CARGO_MANIFEST_DIR" || key === "CARGO_MANIFEST_PATH") {
      const realVal = safeRealpath(value);
      if (realVal === undefined || !isInside(registrySrcReal, realVal)) {
        return undefined;
      }
      canonicalEnv[key] =
        `<REGISTRY_SRC>/${relative(registrySrcReal, realVal).split(sep).join("/")}`;
      hashedEnvNames.add(key);
      continue;
    }
    if (
      EXACT_ENV_KEYS.includes(key) ||
      ENV_PREFIXES.some((prefix) => key.startsWith(prefix))
    ) {
      if (
        value.includes(profileDir) ||
        (worktreeRoot !== "" && value.includes(worktreeRoot))
      ) {
        return undefined;
      }
      canonicalEnv[key] = value;
      hashedEnvNames.add(key);
    }
  }
  const outDirEnv = env.OUT_DIR;
  let outDirEnvReal;
  let outDirHash = "none";
  if (typeof outDirEnv === "string" && outDirEnv.length > 0) {
    const outDirEnvResolved = resolve(outDirEnv);
    if (!isInside(profileDir, outDirEnvResolved)) {
      return undefined;
    }
    outDirEnvReal = safeRealpath(outDirEnvResolved) ?? outDirEnvResolved;
    const hashed = hashDirectoryBounded(
      outDirEnvReal,
      policy.maxOutDirBytes,
      policy.maxOutDirFiles,
    );
    if (!hashed.ok) {
      return undefined;
    }
    outDirHash = hashed.hash;
  }
  const cwdReal = safeRealpath(cwd) ?? resolve(cwd);
  let canonicalCwd;
  if (isInside(registrySrcReal, cwdReal)) {
    canonicalCwd = `<REGISTRY_SRC>/${relative(registrySrcReal, cwdReal).split(sep).join("/")}`;
  } else if (worktreeRoot !== "" && isInside(worktreeRoot, cwdReal)) {
    canonicalCwd = "<WORKTREE_ROOT>";
  } else {
    return undefined;
  }
  return {
    outDir: layout.outDir,
    outDirArgIndex,
    outDirInline,
    profileDir,
    worktreeRoot,
    registrySrcReal,
    outDirEnv:
      typeof outDirEnv === "string" && outDirEnv.length > 0
        ? outDirEnv
        : undefined,
    outDirEnvReal,
    sourceReal,
    sourceRel,
    sourceSha256,
    canonicalArgs,
    canonicalEnv,
    hashedEnvNames,
    outDirHash,
    canonicalCwd,
  };
};

export const cacheKeyFor = (rustcVersion, parsed) =>
  sha256Buffer(
    Buffer.from(
      JSON.stringify({
        schemaVersion: 1,
        rustcVersion,
        sourceRel: parsed.sourceRel,
        sourceSha256: parsed.sourceSha256,
        canonicalArgs: parsed.canonicalArgs,
        canonicalEnv: parsed.canonicalEnv,
        outDirHash: parsed.outDirHash,
        canonicalCwd: parsed.canonicalCwd,
      }),
    ),
  );

const parseDepInfo = (text) => {
  const files = new Set();
  const envDeps = new Map();
  const lines = text.replace(/\\\r?\n/gu, " ").split(/\r?\n/u);
  for (const line of lines) {
    if (line.startsWith("# env-dep:")) {
      const rest = line.slice("# env-dep:".length);
      const eq = rest.indexOf("=");
      if (eq === -1) {
        envDeps.set(rest, null);
      } else {
        envDeps.set(rest.slice(0, eq), rest.slice(eq + 1));
      }
      continue;
    }
    const colon = line.indexOf(": ");
    if (colon === -1) {
      continue;
    }
    const rhs = line.slice(colon + 2);
    let current = "";
    for (let index = 0; index < rhs.length; index += 1) {
      const ch = rhs[index];
      if (ch === "\\" && index + 1 < rhs.length && rhs[index + 1] === " ") {
        current += " ";
        index += 1;
      } else if (ch === " " || ch === "\t") {
        if (current.length > 0) {
          files.add(current);
          current = "";
        }
      } else {
        current += ch;
      }
    }
    if (current.length > 0) {
      files.add(current);
    }
  }
  return { files: [...files].sort(), envDeps };
};

const copyArtifactAtomic = (srcPath, dstPath, mode) => {
  const tempPath = join(
    dirname(dstPath),
    `.h68-restore-${process.pid}-${randomBytes(4).toString("hex")}-${basename(dstPath)}`,
  );
  try {
    try {
      copyFileSync(srcPath, tempPath, constants.COPYFILE_FICLONE);
    } catch {
      copyFileSync(srcPath, tempPath);
    }
    chmodSync(tempPath, mode);
    renameSync(tempPath, dstPath);
  } catch (error) {
    rmSync(tempPath, { force: true });
    throw error;
  }
};

const writeArtifactAtomic = (dstPath, content, mode) => {
  const tempPath = join(
    dirname(dstPath),
    `.h68-restore-${process.pid}-${randomBytes(4).toString("hex")}-${basename(dstPath)}`,
  );
  try {
    writeFileSync(tempPath, content);
    chmodSync(tempPath, mode);
    renameSync(tempPath, dstPath);
  } catch (error) {
    rmSync(tempPath, { force: true });
    throw error;
  }
};

const tryRestoreFromCache = (cacheDir, key, rustcVersion, parsed, env) => {
  const entryDir = join(cacheDir, "entries", key.slice(0, 2), key);
  const manifestPath = join(entryDir, "manifest.json");
  if (!existsSync(manifestPath)) {
    return undefined;
  }
  try {
    const manifestStat = lstatSync(manifestPath);
    if (manifestStat.isSymbolicLink() || !manifestStat.isFile()) {
      return undefined;
    }
    const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
    if (
      manifest?.schemaVersion !== 1 ||
      manifest.key !== key ||
      manifest.rustcVersion !== rustcVersion ||
      !Array.isArray(manifest.depFiles) ||
      !Array.isArray(manifest.envDeps) ||
      !Array.isArray(manifest.outputs) ||
      typeof manifest.stdout !== "string" ||
      typeof manifest.stderr !== "string"
    ) {
      return undefined;
    }
    for (const dep of manifest.depFiles) {
      if (
        dep?.root !== "registry" ||
        typeof dep.rel !== "string" ||
        typeof dep.sha256 !== "string"
      ) {
        return undefined;
      }
      const depPath = resolve(parsed.registrySrcReal, dep.rel);
      if (!isInside(parsed.registrySrcReal, depPath)) {
        return undefined;
      }
      if (sha256File(depPath) !== dep.sha256) {
        return undefined;
      }
    }
    for (const envDep of manifest.envDeps) {
      if (typeof envDep?.name !== "string") {
        return undefined;
      }
      if (envDep.name === "OUT_DIR") {
        continue;
      }
      const actual = env[envDep.name] ?? null;
      if (actual !== envDep.value) {
        return undefined;
      }
    }
    const filesDir = join(entryDir, "files");
    const verifiedOutputs = [];
    let hasDepInfo = false;
    for (const output of manifest.outputs) {
      if (
        !isSafeArtifactName(output?.name) ||
        (output.kind !== "raw" && output.kind !== "dep-info") ||
        typeof output.sha256 !== "string" ||
        typeof output.mode !== "number"
      ) {
        return undefined;
      }
      if (output.kind === "dep-info") {
        hasDepInfo = true;
      }
      const storedPath = join(filesDir, output.name);
      const st = lstatSync(storedPath);
      if (st.isSymbolicLink() || !st.isFile()) {
        return undefined;
      }
      if (sha256File(storedPath) !== output.sha256) {
        return undefined;
      }
      verifiedOutputs.push({ ...output, storedPath });
    }
    if (!hasDepInfo) {
      return undefined;
    }
    const renderTemplate = (text) => {
      if (
        text.includes(BUILD_OUT_DIR_TOKEN) &&
        parsed.outDirEnv === undefined
      ) {
        return undefined;
      }
      let rendered = text.split(OUT_DIR_TOKEN).join(parsed.outDir);
      if (parsed.outDirEnv !== undefined) {
        rendered = rendered.split(BUILD_OUT_DIR_TOKEN).join(parsed.outDirEnv);
      }
      return rendered;
    };
    const renderedStdout = renderTemplate(manifest.stdout);
    const renderedStderr = renderTemplate(manifest.stderr);
    if (renderedStdout === undefined || renderedStderr === undefined) {
      return undefined;
    }
    const renderedDepInfos = new Map();
    for (const output of verifiedOutputs) {
      if (output.kind === "dep-info") {
        const rendered = renderTemplate(
          readFileSync(output.storedPath, "utf8"),
        );
        if (rendered === undefined) {
          return undefined;
        }
        renderedDepInfos.set(output.name, rendered);
      }
    }
    mkdirSync(parsed.outDir, { recursive: true });
    for (const output of verifiedOutputs) {
      const dstPath = join(parsed.outDir, output.name);
      if (output.kind === "dep-info") {
        writeArtifactAtomic(
          dstPath,
          renderedDepInfos.get(output.name),
          output.mode,
        );
      } else {
        copyArtifactAtomic(output.storedPath, dstPath, output.mode);
      }
    }
    try {
      const now = new Date();
      utimesSync(manifestPath, now, now);
    } catch {
      ignoreBestEffortError();
    }
    return {
      stdout: renderedStdout,
      stderr: renderedStderr,
    };
  } catch {
    return undefined;
  }
};

const publishToCache = (
  cacheDir,
  key,
  rustcVersion,
  parsed,
  stageDir,
  stageReplacements,
  stdout,
  stderr,
  env,
) => {
  const stageEntries = readdirSync(stageDir, { withFileTypes: true }).sort(
    (left, right) => left.name.localeCompare(right.name),
  );
  const buildOutDirReplacements = [
    ...new Set(
      [parsed.outDirEnv, parsed.outDirEnvReal].filter(
        (item) => typeof item === "string" && item.length > 0,
      ),
    ),
  ].sort((left, right) => right.length - left.length);
  const templatize = (text) => {
    const step1 = replaceTokens(text, stageReplacements, OUT_DIR_TOKEN);
    return replaceTokens(step1, buildOutDirReplacements, BUILD_OUT_DIR_TOKEN);
  };
  const leaksWorktreePath = (text) =>
    text.includes(parsed.profileDir) ||
    (parsed.worktreeRoot !== "" && text.includes(parsed.worktreeRoot));
  const outputs = [];
  const depFilePaths = new Set();
  const envDepEntries = new Map();
  let hasDepInfo = false;
  for (const entry of stageEntries) {
    if (!isSafeArtifactName(entry.name)) {
      return false;
    }
    const fullPath = join(stageDir, entry.name);
    const st = lstatSync(fullPath);
    if (st.isSymbolicLink() || !st.isFile()) {
      return false;
    }
    const mode = st.mode & 0o777;
    if (entry.name.endsWith(".d")) {
      hasDepInfo = true;
      const rawDepText = readFileSync(fullPath, "utf8");
      const { files, envDeps } = parseDepInfo(rawDepText);
      for (const file of files) {
        depFilePaths.add(file);
      }
      for (const [name, value] of envDeps.entries()) {
        envDepEntries.set(name, value);
      }
      const template = templatize(rawDepText);
      if (leaksWorktreePath(template)) {
        return false;
      }
      outputs.push({
        name: entry.name,
        kind: "dep-info",
        mode,
        template,
      });
    } else {
      outputs.push({
        name: entry.name,
        kind: "raw",
        mode,
        fullPath,
      });
    }
  }
  if (!hasDepInfo) {
    return false;
  }
  const depFiles = [];
  for (const depPath of [...depFilePaths].sort()) {
    const resolved = isAbsolute(depPath)
      ? depPath
      : resolve(process.cwd(), depPath);
    const depReal = safeRealpath(resolved);
    if (depReal === undefined) {
      return false;
    }
    if (
      parsed.outDirEnvReal !== undefined &&
      isInside(parsed.outDirEnvReal, depReal)
    ) {
      continue;
    }
    if (!isInside(parsed.registrySrcReal, depReal)) {
      return false;
    }
    depFiles.push({
      root: "registry",
      rel: relative(parsed.registrySrcReal, depReal).split(sep).join("/"),
      sha256: sha256File(depReal),
    });
  }
  const envDeps = [];
  for (const [name, depValue] of [...envDepEntries.entries()].sort((a, b) =>
    a[0].localeCompare(b[0]),
  )) {
    if (name === "OUT_DIR") {
      continue;
    }
    const actual = env[name] ?? null;
    if (actual !== depValue) {
      return false;
    }
    if (
      actual !== null &&
      (actual.includes(parsed.profileDir) ||
        (parsed.worktreeRoot !== "" && actual.includes(parsed.worktreeRoot)))
    ) {
      return false;
    }
    envDeps.push({ name, value: actual });
  }
  const templateStdout = templatize(stdout);
  const templateStderr = templatize(stderr);
  if (leaksWorktreePath(templateStdout) || leaksWorktreePath(templateStderr)) {
    return false;
  }
  const tmpRoot = join(cacheDir, ".tmp");
  const entryDir = join(cacheDir, "entries", key.slice(0, 2), key);
  if (existsSync(join(entryDir, "manifest.json"))) {
    return true;
  }
  const tmpDir = join(
    tmpRoot,
    `${key}-${process.pid}-${randomBytes(4).toString("hex")}`,
  );
  try {
    mkdirSync(join(tmpDir, "files"), { recursive: true });
    mkdirSync(dirname(entryDir), { recursive: true });
    const manifestOutputs = [];
    for (const output of outputs) {
      const targetFile = join(tmpDir, "files", output.name);
      if (output.kind === "dep-info") {
        writeFileSync(targetFile, output.template);
        chmodSync(targetFile, output.mode);
      } else {
        try {
          copyFileSync(output.fullPath, targetFile, constants.COPYFILE_FICLONE);
        } catch {
          copyFileSync(output.fullPath, targetFile);
        }
        chmodSync(targetFile, output.mode);
      }
      manifestOutputs.push({
        name: output.name,
        kind: output.kind,
        mode: output.mode,
        sha256: sha256File(targetFile),
      });
    }
    const manifest = {
      schemaVersion: 1,
      key,
      rustcVersion,
      depFiles,
      envDeps,
      outputs: manifestOutputs,
      stdout: templateStdout,
      stderr: templateStderr,
    };
    writeFileSync(join(tmpDir, "manifest.json"), JSON.stringify(manifest));
    try {
      renameSync(tmpDir, entryDir);
    } catch {
      rmSync(tmpDir, { recursive: true, force: true });
    }
    return true;
  } catch {
    rmSync(tmpDir, { recursive: true, force: true });
    return false;
  }
};

const execDirect = (rustc, args) => {
  const result = spawnSync(rustc, args, { stdio: "inherit" });
  if (result.error !== undefined) {
    process.stderr.write(`rustc-wrapper: ${result.error.message}\n`);
    return 1;
  }
  return result.status ?? 1;
};

const runWrapper = (argv) => {
  const [rustc, ...args] = argv;
  if (rustc === undefined) {
    return 1;
  }
  const policy = readPolicy();
  const parsed = parseInvocation({ rustc, args, policy });
  if (parsed === undefined) {
    return execDirect(rustc, args);
  }
  const versionRun = spawnSync(rustc, ["-vV"], { encoding: "utf8" });
  if (versionRun.status !== 0) {
    return execDirect(rustc, args);
  }
  const rustcVersion = versionRun.stdout.trim();
  const key = cacheKeyFor(rustcVersion, parsed);
  const cacheDir = resolveCacheDir(process.env, policy);
  const restored = tryRestoreFromCache(
    cacheDir,
    key,
    rustcVersion,
    parsed,
    process.env,
  );
  if (restored !== undefined) {
    if (restored.stdout.length > 0) {
      process.stdout.write(restored.stdout);
    }
    if (restored.stderr.length > 0) {
      process.stderr.write(restored.stderr);
    }
    return 0;
  }
  const stageRoot = join(parsed.profileDir, ".h68-stage");
  const stageDir = join(
    stageRoot,
    `${process.pid}-${randomBytes(6).toString("hex")}`,
  );
  mkdirSync(stageDir, { recursive: true });
  const stageReal = safeRealpath(stageDir) ?? stageDir;
  const stageReplacements = [...new Set([stageDir, stageReal])].sort(
    (left, right) => right.length - left.length,
  );
  const stagedArgs = [...args];
  if (parsed.outDirInline) {
    stagedArgs[parsed.outDirArgIndex] = `--out-dir=${stageDir}`;
  } else {
    stagedArgs[parsed.outDirArgIndex + 1] = stageDir;
  }
  try {
    const compiled = spawnSync(rustc, stagedArgs, {
      encoding: "utf8",
      maxBuffer: 64 * 1024 * 1024,
    });
    const stdout = compiled.stdout ?? "";
    const stderr = compiled.stderr ?? "";
    if (compiled.status !== 0) {
      if (stdout.length > 0) {
        process.stdout.write(
          replaceTokens(stdout, stageReplacements, parsed.outDir),
        );
      }
      if (stderr.length > 0) {
        process.stderr.write(
          replaceTokens(stderr, stageReplacements, parsed.outDir),
        );
      }
      return compiled.status ?? 1;
    }
    publishToCache(
      cacheDir,
      key,
      rustcVersion,
      parsed,
      stageDir,
      stageReplacements,
      stdout,
      stderr,
      process.env,
    );
    mkdirSync(parsed.outDir, { recursive: true });
    for (const entry of readdirSync(stageDir, { withFileTypes: true })) {
      const srcPath = join(stageDir, entry.name);
      const dstPath = join(parsed.outDir, entry.name);
      if (entry.isDirectory()) {
        cpSync(srcPath, dstPath, { recursive: true, force: true });
      } else if (entry.name.endsWith(".d")) {
        const rendered = replaceTokens(
          readFileSync(srcPath, "utf8"),
          stageReplacements,
          parsed.outDir,
        );
        const mode = statSync(srcPath).mode & 0o777;
        writeArtifactAtomic(dstPath, rendered, mode);
      } else {
        const mode = statSync(srcPath).mode & 0o777;
        copyArtifactAtomic(srcPath, dstPath, mode);
      }
    }
    if (stdout.length > 0) {
      process.stdout.write(
        replaceTokens(stdout, stageReplacements, parsed.outDir),
      );
    }
    if (stderr.length > 0) {
      process.stderr.write(
        replaceTokens(stderr, stageReplacements, parsed.outDir),
      );
    }
    return 0;
  } finally {
    rmSync(stageDir, { recursive: true, force: true });
    try {
      if (readdirSync(stageRoot).length === 0) {
        rmdirSync(stageRoot);
      }
    } catch {
      ignoreBestEffortError();
    }
  }
};

if (
  process.argv[1] !== undefined &&
  resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  process.exitCode = runWrapper(process.argv.slice(2));
}
