import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { isDeepStrictEqual } from "node:util";

export const STEPS_FILE = "harness/check-list/steps.json";

export const JUSTFILE_EXPECTED = "harness/check-list/justfile.json";

export const KINDS = ["check", "integrity", "runner", "setup", "report"];

const FIELDS = ["recipe", "command", "kind", "cases", "test"];

const TEST_FIELDS = ["file", "name"];

const RUST_SOURCE = "crates/xtask/src/";

const RUST_SUITE = "xtask::bin/xtask";

export const readSteps = (root) =>
  JSON.parse(readFileSync(join(root, STEPS_FILE), "utf8"));

export const readExpectedJustfile = (root) =>
  JSON.parse(readFileSync(join(root, JUSTFILE_EXPECTED), "utf8"));

export const dumpJustfile = (justfile) => {
  const run = spawnSync(
    "just",
    [
      "--justfile",
      justfile,
      "--working-directory",
      dirname(justfile),
      "--dump",
      "--dump-format",
      "json",
    ],
    { encoding: "utf8" },
  );
  if (run.status !== 0) {
    return { problem: run.stderr.trim() };
  }
  const dump = JSON.parse(run.stdout);
  return Object.fromEntries(
    Object.entries(dump).filter(([key]) => key !== "source"),
  );
};

export const justfileProblems = (dump, expected) => {
  if (dump.problem !== undefined) {
    return [`just가 justfile을 읽지 못했다: ${dump.problem}`];
  }
  const keys = [...new Set([...Object.keys(dump), ...Object.keys(expected)])];
  return keys.flatMap((key) => {
    if (key !== "recipes") {
      return isDeepStrictEqual(dump[key], expected[key])
        ? []
        : [`justfile의 ${key}가 기대값과 다르다`];
    }
    const names = [
      ...new Set([
        ...Object.keys(dump.recipes ?? {}),
        ...Object.keys(expected.recipes ?? {}),
      ]),
    ].sort();
    return names
      .filter(
        (name) =>
          !isDeepStrictEqual(dump.recipes?.[name], expected.recipes?.[name]),
      )
      .map((name) => `justfile 레시피 ${name}가 기대값과 다르다`);
  });
};

const expression = (node) => {
  if (!Array.isArray(node)) {
    return JSON.stringify(node);
  }
  const [kind, ...rest] = node;
  if (kind === "call") {
    const [name, ...args] = rest;
    return `${name}(${args.map(expression).join(", ")})`;
  }
  if (kind === "variable") {
    return rest[0];
  }
  return JSON.stringify(node);
};

const fragment = (part) =>
  typeof part === "string" ? part : `{{${part.map(expression).join("")}}}`;

export const renderLine = (line) => line.map(fragment).join("");

export const checkSteps = (dump, top = "check") => {
  const recipes = dump.recipes ?? {};
  const seen = new Set();
  const steps = [];
  const visit = (name) => {
    if (seen.has(name)) {
      return;
    }
    seen.add(name);
    const recipe = recipes[name];
    if (recipe === undefined) {
      steps.push({ recipe: name, command: undefined });
      return;
    }
    recipe.dependencies.forEach((dependency) => visit(dependency.recipe));
    recipe.body.forEach((line) =>
      steps.push({ recipe: name, command: renderLine(line) }),
    );
  };
  visit(top);
  return steps;
};

const stepKey = (step) => `${step.recipe} | ${step.command}`;

const escaped = (text) => text.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&");

export const runnerGlobs = (dump) =>
  checkSteps(dump)
    .map((step) => /^node --test '([^']+)'$/u.exec(step.command ?? ""))
    .filter((match) => match !== null)
    .map((match) => match[1]);

export const globMatches = (glob, path) =>
  new RegExp(`^${glob.split("*").map(escaped).join("[^/]*")}$`, "u").test(path);

export const rustTestId = (file, name) =>
  `${file
    .slice(RUST_SOURCE.length)
    .replace(/\.rs$/u, "")
    .split("/")
    .join("::")}::${name}`;

export const runnableRustTests = (root) => {
  const run = spawnSync(
    "cargo",
    [
      "nextest",
      "list",
      "--locked",
      "--package",
      "xtask",
      "--message-format",
      "json",
    ],
    {
      cwd: root,
      encoding: "utf8",
      maxBuffer: 64 * 1024 * 1024,
      env: {
        ...process.env,
        RUSTFLAGS: "-D warnings",
        RUSTC_WRAPPER: "",
        RUSTC_WORKSPACE_WRAPPER: "",
      },
    },
  );
  if (run.status !== 0) {
    return { problem: run.stderr.trim().split("\n").slice(-5).join("\n") };
  }
  const cases =
    JSON.parse(run.stdout)["rust-suites"]?.[RUST_SUITE]?.testcases ?? {};
  return {
    tests: new Set(
      Object.entries(cases)
        .filter(([, entry]) => entry.ignored === false)
        .map(([id]) => id),
    ),
  };
};

const RUST_ITEM =
  /^(?:#\[test\]\s*\n)?(?:pub(?:\([^)]*\))? )?(?:const|static|fn|struct|enum|impl|mod|type)\b[^\n]*/gmu;

const JS_ITEM =
  /^(?:export )?(?:const|let|function|async function|test\()[^\n]*/gmu;

const itemNames = (text) => {
  const pattern =
    /(?:const|static|fn|struct|enum|type|let|function)\s+(?:([A-Za-z_][A-Za-z0-9_]*)|[{[]([^}\]]*)[}\]])/u.exec(
      text,
    );
  if (pattern === null) {
    return [];
  }
  return pattern[1] === undefined
    ? [...pattern[2].matchAll(/[A-Za-z_][A-Za-z0-9_]*/gu)].map(
        (match) => match[0],
      )
    : [pattern[1]];
};

const itemsOf = (source, pattern) => {
  const starts = [...source.matchAll(pattern)].map((match) => match.index);
  return starts.map((start, index) => {
    const text = source.slice(start, starts[index + 1] ?? source.length);
    return { names: itemNames(text.split("\n").slice(0, 2).join(" ")), text };
  });
};

const jsImports = (root, file, seen = new Set()) => {
  if (seen.has(file) || !existsSync(join(root, file))) {
    return [];
  }
  seen.add(file);
  const source = readFileSync(join(root, file), "utf8");
  const imported = [...source.matchAll(/from "(\.[^"]+)"/gu)].map((match) =>
    relative(root, resolve(join(root, dirname(file)), match[1])),
  );
  return [file, ...imported.flatMap((path) => jsImports(root, path, seen))];
};

const rustNeighbours = (file) => {
  const parent = file.endsWith("/tests.rs")
    ? `${dirname(file)}.rs`
    : /_tests\.rs$/u.test(file) && dirname(file) !== RUST_SOURCE.slice(0, -1)
      ? `${dirname(file)}.rs`
      : undefined;
  return parent === undefined ? [file] : [file, parent];
};

const mentions = (text, name) =>
  new RegExp(`\\b${escaped(name)}\\b`, "u").test(text);

const testItem = (items, reference) =>
  reference.file.endsWith(".rs")
    ? items.find(
        (item) =>
          item.text.startsWith("#[test]") &&
          item.names.includes(reference.name),
      )
    : items.find((item) =>
        new RegExp(
          `^test\\(\\s*${escaped(JSON.stringify(reference.name))}`,
          "u",
        ).test(item.text),
      );

const quoted = (text, literal) => text.includes(JSON.stringify(literal));

const mentionsPath = (item, cases) => {
  const segments = cases.split("/");
  const last = segments.at(-1) ?? cases;
  const parent = segments.at(-2) ?? "";
  return (
    item.text.includes(cases) ||
    (quoted(item.text, parent) && quoted(item.text, last)) ||
    (dirname(item.file) === dirname(cases) && quoted(item.text, last))
  );
};

export const readsCases = (root, step) => {
  const reference = step.test;
  if (step.cases === reference.file) {
    return true;
  }
  const rust = reference.file.endsWith(".rs");
  const files = (
    rust ? rustNeighbours(reference.file) : jsImports(root, reference.file)
  ).filter((file) => existsSync(join(root, file)));
  const items = files.flatMap((file) =>
    itemsOf(
      readFileSync(join(root, file), "utf8"),
      rust ? RUST_ITEM : JS_ITEM,
    ).map((item) => ({ ...item, file })),
  );
  const own = testItem(
    items.filter((item) => item.file === reference.file),
    reference,
  );
  if (own === undefined) {
    return false;
  }
  const carriers = new Set(
    items
      .filter((item) => item.file === step.cases)
      .flatMap((item) => item.names),
  );
  let grown = true;
  while (grown) {
    grown = false;
    for (const item of items) {
      const fresh = item.names.filter((name) => !carriers.has(name));
      if (
        fresh.length > 0 &&
        (mentionsPath(item, step.cases) ||
          [...carriers].some((name) => mentions(item.text, name)))
      ) {
        fresh.forEach((name) => carriers.add(name));
        grown = true;
      }
    }
  }
  return (
    mentionsPath(own, step.cases) ||
    [...carriers].some((name) => mentions(own.text, name))
  );
};

const testRuns = (root, reference, runnable, globs) => {
  if (reference.file.endsWith(".rs")) {
    return (
      reference.file.startsWith(RUST_SOURCE) &&
      runnable.tests !== undefined &&
      runnable.tests.has(rustTestId(reference.file, reference.name))
    );
  }
  return (
    globs.some((glob) => globMatches(glob, reference.file)) &&
    existsSync(join(root, reference.file)) &&
    new RegExp(
      `^test\\(\\s*${escaped(JSON.stringify(reference.name))}`,
      "mu",
    ).test(readFileSync(join(root, reference.file), "utf8"))
  );
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

export const listProblems = (root, list, dump, runnable) => {
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
  const ran = checkSteps(dump).map(stepKey);
  const globs = runnerGlobs(dump);
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
  const running = list.steps
    .filter((step) => !testRuns(root, step.test, runnable, globs))
    .map(
      (step) =>
        `${stepKey(step)}: 자체 테스트 ${step.test.file} "${step.test.name}"가 실행 목록에 없다`,
    );
  const reading = list.steps
    .filter((step) => !readsCases(root, step))
    .map(
      (step) =>
        `${stepKey(step)}: 자체 테스트 "${step.test.name}"가 예제 위치 ${step.cases}를 읽지 않는다`,
    );
  return [
    ...(runnable.problem === undefined
      ? []
      : [`cargo nextest list가 실패했다: ${runnable.problem}`]),
    ...duplicates,
    ...unlisted,
    ...stale,
    ...cases,
    ...running,
    ...reading,
  ];
};
