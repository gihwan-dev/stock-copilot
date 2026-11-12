import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";

export const STEPS_FILE = "harness/check-list/steps.json";

export const KINDS = ["check", "integrity", "runner", "setup", "report"];

const FIELDS = ["recipe", "command", "kind", "cases", "test"];

const TEST_FIELDS = ["file", "name"];

export const readSteps = (root) =>
  JSON.parse(readFileSync(join(root, STEPS_FILE), "utf8"));

const recipeHeader = /^([A-Za-z0-9_-]+)(?:\s+[^:]*)?:(?!=)(.*)$/u;

export const recipes = (justfile) => {
  const table = new Map();
  let current;
  for (const line of justfile.split("\n")) {
    const header = recipeHeader.exec(line);
    if (header !== null && !line.startsWith(" ")) {
      current = {
        dependencies: header[2]
          .trim()
          .split(/\s+/u)
          .filter((word) => word !== ""),
        lines: [],
      };
      table.set(header[1], current);
    } else if (line.startsWith("    ") && current !== undefined) {
      current.lines.push(line.slice(4));
    } else if (line.trim() === "") {
      current = undefined;
    }
  }
  return table;
};

export const checkSteps = (justfile, top = "check") => {
  const table = recipes(justfile);
  const seen = new Set();
  const steps = [];
  const visit = (name) => {
    if (seen.has(name)) {
      return;
    }
    seen.add(name);
    const recipe = table.get(name);
    if (recipe === undefined) {
      steps.push({ recipe: name, command: undefined });
      return;
    }
    recipe.dependencies.forEach(visit);
    recipe.lines.forEach((command) => steps.push({ recipe: name, command }));
  };
  visit(top);
  return steps;
};

const stepKey = (step) => `${step.recipe} | ${step.command}`;

const escaped = (text) => text.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&");

export const testExists = (root, reference) => {
  const path = join(root, reference.file);
  if (!existsSync(path)) {
    return false;
  }
  const source = readFileSync(path, "utf8");
  const pattern = reference.file.endsWith(".rs")
    ? new RegExp(
        `#\\[test\\]\\s*(?:#\\[[^\\]]*\\]\\s*)*fn ${escaped(reference.name)}\\(`,
        "u",
      )
    : new RegExp(
        `\\btest\\(\\s*${escaped(JSON.stringify(reference.name))}`,
        "u",
      );
  return pattern.test(source);
};

const shapeProblems = (step, index) => {
  const label = `steps[${index}]`;
  const missing = FIELDS.filter((field) => !(field in step)).map(
    (field) => `${label}: ${field} 칸이 없다`,
  );
  const extra = Object.keys(step)
    .filter((field) => !FIELDS.includes(field))
    .map((field) => `${label}: 모르는 칸 ${field}`);
  const kind =
    "kind" in step && !KINDS.includes(step.kind)
      ? [`${label}: 종류 ${step.kind}는 ${KINDS.join(", ")} 밖이다`]
      : [];
  const test =
    "test" in step &&
    (typeof step.test !== "object" ||
      step.test === null ||
      TEST_FIELDS.some((field) => typeof step.test[field] !== "string") ||
      Object.keys(step.test).length !== TEST_FIELDS.length)
      ? [`${label}: test는 file과 name만 가진다`]
      : [];
  return [...missing, ...extra, ...kind, ...test];
};

export const listProblems = (root, list, justfile) => {
  if (
    typeof list !== "object" ||
    list === null ||
    !Array.isArray(list.steps) ||
    Object.keys(list).length !== 1
  ) {
    return [`${STEPS_FILE}: 최상위는 steps 배열 하나다`];
  }
  const shapes = list.steps.flatMap(shapeProblems);
  if (shapes.length > 0) {
    return shapes;
  }
  const listed = list.steps.map(stepKey);
  const ran = checkSteps(justfile).map(stepKey);
  const duplicates = listed
    .filter((key, index) => listed.indexOf(key) !== index)
    .map((key) => `목록에 두 번 있다: ${key}`);
  const unlisted = ran
    .filter((key) => !listed.includes(key))
    .map((key) => `just check가 돌지만 목록에 없다: ${key}`);
  const stale = listed
    .filter((key) => !ran.includes(key))
    .map((key) => `목록에 있지만 just check가 돌지 않는다: ${key}`);
  const cases = list.steps
    .filter((step) => !existsSync(join(root, step.cases)))
    .map((step) => `${stepKey(step)}: 예제 위치 ${step.cases}가 없다`);
  const tests = list.steps
    .filter((step) => !testExists(root, step.test))
    .map(
      (step) =>
        `${stepKey(step)}: 자체 테스트 ${step.test.file} "${step.test.name}"가 없다`,
    );
  return [...duplicates, ...unlisted, ...stale, ...cases, ...tests];
};

export const runnerGlobs = (justfile) =>
  checkSteps(justfile)
    .map((step) => /^node --test '([^']+)'$/u.exec(step.command ?? ""))
    .filter((match) => match !== null)
    .map((match) => match[1]);

export const globMatches = (glob, path) =>
  new RegExp(`^${glob.split("*").map(escaped).join("[^/]*")}$`, "u").test(path);
