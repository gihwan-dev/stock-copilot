import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { packageOfSource } from "../ci/diff-mutants-plan.mjs";

export const TARGETS_FILE = "harness/mutants/targets.toml";
export const MUTANTS_CONFIG_FILE = ".cargo/mutants.toml";
export const ALLOWED_CATEGORIES = new Set([
  "money_quantity_fx",
  "order_decision_execution",
  "secret_auth_boundary",
]);

const decodeTomlString = (raw) => {
  if (!raw.startsWith('"') || !raw.endsWith('"')) {
    return undefined;
  }
  try {
    const value = JSON.parse(raw);
    return typeof value === "string" ? value : undefined;
  } catch {
    return undefined;
  }
};

export const parseTargetsToml = (text) => {
  const entries = [];
  let current;
  for (const rawLine of text.split(/\r?\n/u)) {
    const line = rawLine.trim();
    if (line === "") {
      continue;
    }
    if (line === "[[target]]") {
      current = {};
      entries.push(current);
      continue;
    }
    if (current === undefined) {
      throw new Error(`${TARGETS_FILE}: [[target]] 밖의 줄: ${line}`);
    }
    const match = /^([a-z_]+)\s*=\s*(.+)$/u.exec(line);
    if (match === null) {
      throw new Error(`${TARGETS_FILE}: 해석할 수 없는 줄: ${line}`);
    }
    const [, key, rawValue] = match;
    if (!["path", "category", "reason"].includes(key)) {
      throw new Error(`${TARGETS_FILE}: 허용되지 않은 키: ${key}`);
    }
    if (key in current) {
      throw new Error(`${TARGETS_FILE}: 중복 키: ${key}`);
    }
    const value = decodeTomlString(rawValue.trim());
    if (value === undefined || value.trim().length === 0) {
      throw new Error(
        `${TARGETS_FILE}: ${key}는 비어 있지 않은 문자열이어야 한다`,
      );
    }
    current[key] = value;
  }
  return entries;
};

export const parseExamineGlobs = (text) => {
  const match = /examine_globs\s*=\s*\[([\s\S]*?)\]/u.exec(text);
  if (match === null) {
    throw new Error(`${MUTANTS_CONFIG_FILE}: examine_globs 배열이 없다`);
  }
  const inner = match[1];
  const globs = [];
  for (const rawLine of inner.split(/\r?\n/u)) {
    const line = rawLine.trim();
    if (line === "") {
      continue;
    }
    const cleaned = line.endsWith(",") ? line.slice(0, -1).trim() : line;
    const value = decodeTomlString(cleaned);
    if (value === undefined) {
      throw new Error(
        `${MUTANTS_CONFIG_FILE}: 잘못된 examine_globs 항목: ${line}`,
      );
    }
    globs.push(value);
  }
  return globs;
};

const directoryHasRs = (directory) => {
  if (!existsSync(directory) || !statSync(directory).isDirectory()) {
    return false;
  }
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const full = join(directory, entry.name);
    if (entry.isDirectory()) {
      if (directoryHasRs(full)) {
        return true;
      }
    } else if (entry.isFile() && entry.name.endsWith(".rs")) {
      return true;
    }
  }
  return false;
};

const pathExistsOnDisk = (root, targetPath) => {
  if (targetPath.endsWith("/**/*.rs")) {
    const prefix = targetPath.slice(0, -"/**/*.rs".length);
    return directoryHasRs(join(root, prefix));
  }
  const full = join(root, targetPath);
  return (
    targetPath.endsWith(".rs") && existsSync(full) && statSync(full).isFile()
  );
};

const sampleFileForOwner = (targetPath) =>
  targetPath.endsWith("/**/*.rs")
    ? `${targetPath.slice(0, -"/**/*.rs".length)}/lib.rs`
    : targetPath;

export const targetProblems = (root) => {
  let entries;
  try {
    entries = parseTargetsToml(readFileSync(join(root, TARGETS_FILE), "utf8"));
  } catch (error) {
    return [error instanceof Error ? error.message : String(error)];
  }
  if (entries.length === 0) {
    return [`${TARGETS_FILE}: 대상 항목이 비어 있다`];
  }
  const problems = [];
  const seen = new Set();
  for (const [index, entry] of entries.entries()) {
    const label = `${TARGETS_FILE}#${index + 1}`;
    for (const required of ["path", "category", "reason"]) {
      if (
        typeof entry[required] !== "string" ||
        entry[required].trim() === ""
      ) {
        problems.push(`${label}: ${required}가 비어 있다`);
      }
    }
    if (!ALLOWED_CATEGORIES.has(entry.category)) {
      problems.push(`${label}: 알 수 없는 분류 '${entry.category}'`);
    }
    if (typeof entry.path === "string") {
      if (seen.has(entry.path)) {
        problems.push(`${label}: 중복 경로 '${entry.path}'`);
      }
      seen.add(entry.path);
      if (!pathExistsOnDisk(root, entry.path)) {
        problems.push(
          `${label}: 경로가 디스크에 존재하지 않는다: ${entry.path}`,
        );
      }
      try {
        packageOfSource(sampleFileForOwner(entry.path));
      } catch {
        problems.push(
          `${label}: 플랫폼·테스트 소유 패키지가 선언되지 않은 경로: ${entry.path}`,
        );
      }
    }
  }
  let configured;
  try {
    configured = parseExamineGlobs(
      readFileSync(join(root, MUTANTS_CONFIG_FILE), "utf8"),
    );
  } catch (error) {
    problems.push(error instanceof Error ? error.message : String(error));
    return problems;
  }
  const expected = entries.map((entry) => entry.path);
  if (
    configured.length !== expected.length ||
    configured.some((glob, index) => glob !== expected[index])
  ) {
    problems.push(
      `${MUTANTS_CONFIG_FILE}의 examine_globs가 ${TARGETS_FILE}과 1:1로 일치하지 않는다`,
    );
  }
  return problems;
};

const repoRoot = resolve(fileURLToPath(import.meta.url), "..", "..", "..");

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const root = resolve(process.argv[2] ?? repoRoot);
  const problems = targetProblems(root);
  problems.forEach((problem) => process.stdout.write(`FAIL ${problem}\n`));
  process.stdout.write(`mutants-targets: ${problems.length} failure(s)\n`);
  process.exitCode = problems.length === 0 ? 0 : 1;
}
