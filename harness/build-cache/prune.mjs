import { randomBytes } from "node:crypto";
import {
  existsSync,
  lstatSync,
  mkdirSync,
  readdirSync,
  renameSync,
  rmSync,
  statSync,
} from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { readPolicy, resolveCacheDir } from "./rustc-wrapper.mjs";

const BYTES_PER_MIB = 1024 * 1024;
const ignoreBestEffortError = () => {};

const directoryBytes = (rootDir) => {
  let bytes = 0;
  const walk = (current) => {
    for (const entry of readdirSync(current, { withFileTypes: true })) {
      const fullPath = join(current, entry.name);
      try {
        const st = lstatSync(fullPath);
        if (st.isDirectory()) {
          walk(fullPath);
        } else {
          bytes += st.size;
        }
      } catch {
        ignoreBestEffortError();
      }
    }
  };
  if (existsSync(rootDir)) {
    walk(rootDir);
  }
  return bytes;
};

const safeTrashDelete = (cacheDir, targetPath) => {
  const trashRoot = join(cacheDir, ".trash");
  mkdirSync(trashRoot, { recursive: true });
  const destination = join(
    trashRoot,
    `${process.pid}-${randomBytes(4).toString("hex")}`,
  );
  try {
    renameSync(targetPath, destination);
    rmSync(destination, { recursive: true, force: true });
    return true;
  } catch {
    return false;
  }
};

const listEntries = (cacheDir) => {
  const entriesRoot = join(cacheDir, "entries");
  if (!existsSync(entriesRoot)) {
    return [];
  }
  const found = [];
  for (const shard of readdirSync(entriesRoot, { withFileTypes: true })) {
    if (!shard.isDirectory()) {
      continue;
    }
    const shardDir = join(entriesRoot, shard.name);
    for (const item of readdirSync(shardDir, { withFileTypes: true })) {
      if (!item.isDirectory()) {
        continue;
      }
      const entryDir = join(shardDir, item.name);
      const manifestPath = join(entryDir, "manifest.json");
      let mtimeMs;
      try {
        mtimeMs = existsSync(manifestPath)
          ? statSync(manifestPath).mtimeMs
          : statSync(entryDir).mtimeMs;
      } catch {
        continue;
      }
      found.push({
        key: item.name,
        dir: entryDir,
        mtimeMs,
        bytes: directoryBytes(entryDir),
      });
    }
  }
  return found;
};

export const cacheStats = (cacheDir, maxMegabytes) => {
  const entries = listEntries(cacheDir);
  const bytes = entries.reduce((sum, entry) => sum + entry.bytes, 0);
  const tmpDir = join(cacheDir, ".tmp");
  const tempEntries = existsSync(tmpDir) ? readdirSync(tmpDir).length : 0;
  return {
    dir: cacheDir,
    entries: entries.length,
    bytes,
    megabytes: Math.round((bytes / BYTES_PER_MIB) * 10) / 10,
    maxMegabytes,
    tempEntries,
  };
};

export const pruneCache = ({
  cacheDir,
  maxMegabytes,
  pruneTargetRatio,
  minEntryAgeSeconds,
  staleTempAgeSeconds,
  clear = false,
  nowMs = Date.now(),
}) => {
  if (!existsSync(cacheDir)) {
    return {
      dir: cacheDir,
      beforeEntries: 0,
      beforeBytes: 0,
      removedEntries: 0,
      removedTemps: 0,
      afterEntries: 0,
      afterBytes: 0,
    };
  }
  const trashRoot = join(cacheDir, ".trash");
  if (existsSync(trashRoot)) {
    for (const entry of readdirSync(trashRoot)) {
      rmSync(join(trashRoot, entry), { recursive: true, force: true });
    }
  }
  let removedTemps = 0;
  const tmpRoot = join(cacheDir, ".tmp");
  if (existsSync(tmpRoot)) {
    for (const entry of readdirSync(tmpRoot)) {
      const fullPath = join(tmpRoot, entry);
      try {
        const ageSeconds = (nowMs - statSync(fullPath).mtimeMs) / 1000;
        if (clear || ageSeconds >= staleTempAgeSeconds) {
          rmSync(fullPath, { recursive: true, force: true });
          removedTemps += 1;
        }
      } catch {
        ignoreBestEffortError();
      }
    }
  }
  const entries = listEntries(cacheDir);
  const beforeEntries = entries.length;
  const beforeBytes = entries.reduce((sum, entry) => sum + entry.bytes, 0);
  const maxBytes = maxMegabytes * BYTES_PER_MIB;
  const targetBytes = Math.floor(maxBytes * pruneTargetRatio);
  let remainingBytes = beforeBytes;
  let removedEntries = 0;
  if (clear) {
    const metaRoot = join(cacheDir, "meta");
    if (existsSync(metaRoot)) {
      rmSync(metaRoot, { recursive: true, force: true });
    }
    for (const entry of entries) {
      if (safeTrashDelete(cacheDir, entry.dir)) {
        remainingBytes -= entry.bytes;
        removedEntries += 1;
      }
    }
  } else if (remainingBytes > maxBytes) {
    const oldestFirst = [...entries].sort(
      (left, right) => left.mtimeMs - right.mtimeMs,
    );
    for (const entry of oldestFirst) {
      if (remainingBytes <= targetBytes) {
        break;
      }
      const ageSeconds = (nowMs - entry.mtimeMs) / 1000;
      if (ageSeconds < minEntryAgeSeconds) {
        continue;
      }
      if (safeTrashDelete(cacheDir, entry.dir)) {
        remainingBytes -= entry.bytes;
        removedEntries += 1;
      }
    }
  }
  return {
    dir: cacheDir,
    beforeEntries,
    beforeBytes,
    removedEntries,
    removedTemps,
    afterEntries: beforeEntries - removedEntries,
    afterBytes: Math.max(0, remainingBytes),
  };
};

const parseCli = (argv, env, policy) => {
  let mode = "prune";
  let cacheDir = resolveCacheDir(env, policy);
  let maxMegabytes = policy.defaultMaxMegabytes;
  let minEntryAgeSeconds = policy.minEntryAgeSeconds;
  if (
    typeof env.STOCK_COPILOT_BUILD_CACHE_MAX_MB === "string" &&
    /^\d+$/u.test(env.STOCK_COPILOT_BUILD_CACHE_MAX_MB)
  ) {
    maxMegabytes = Number(env.STOCK_COPILOT_BUILD_CACHE_MAX_MB);
  }
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--stats") {
      mode = "stats";
    } else if (arg === "--clear") {
      mode = "clear";
    } else if (arg === "--dir" && index + 1 < argv.length) {
      cacheDir = resolve(argv[index + 1]);
      index += 1;
    } else if (arg === "--max-mb" && index + 1 < argv.length) {
      maxMegabytes = Number(argv[index + 1]);
      index += 1;
    } else if (arg === "--min-age-seconds" && index + 1 < argv.length) {
      minEntryAgeSeconds = Number(argv[index + 1]);
      index += 1;
    } else {
      throw new Error(`알 수 없는 인자: ${arg}`);
    }
  }
  if (!Number.isSafeInteger(maxMegabytes) || maxMegabytes < 1) {
    throw new Error(`잘못된 상한(MiB): ${maxMegabytes}`);
  }
  if (!Number.isSafeInteger(minEntryAgeSeconds) || minEntryAgeSeconds < 0) {
    throw new Error(`잘못된 보호 시간(초): ${minEntryAgeSeconds}`);
  }
  return { mode, cacheDir, maxMegabytes, minEntryAgeSeconds };
};

if (
  process.argv[1] !== undefined &&
  resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  const policy = readPolicy();
  const options = parseCli(process.argv.slice(2), process.env, policy);
  if (options.mode === "stats") {
    const stats = cacheStats(options.cacheDir, options.maxMegabytes);
    process.stdout.write(`${JSON.stringify(stats, undefined, 2)}\n`);
  } else {
    const result = pruneCache({
      cacheDir: options.cacheDir,
      maxMegabytes: options.maxMegabytes,
      pruneTargetRatio: policy.pruneTargetRatio,
      minEntryAgeSeconds: options.minEntryAgeSeconds,
      staleTempAgeSeconds: policy.staleTempAgeSeconds,
      clear: options.mode === "clear",
    });
    const afterMib = Math.round((result.afterBytes / BYTES_PER_MIB) * 10) / 10;
    process.stdout.write(
      `build-cache: 항목 ${result.afterEntries}개 (${afterMib} MiB / 상한 ${options.maxMegabytes} MiB, 제거 항목 ${result.removedEntries}개, 임시 ${result.removedTemps}개)\n`,
    );
  }
}
