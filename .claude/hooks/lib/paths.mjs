import {
  existsSync,
  lstatSync,
  readdirSync,
  realpathSync,
  statSync,
} from "node:fs";
import { homedir, tmpdir } from "node:os";
import { basename, dirname, isAbsolute, join } from "node:path";
import {
  isWritable,
  patternMatches,
  patternMatchesIgnoringCase,
} from "./allowlist.mjs";
import { ROLE } from "./roles.mjs";

export const PATH_PROBLEM = Object.freeze({
  missing: "경로가 없다",
  relative: "절대 경로가 아니다",
  unnormalized: "`.`, `..`, 빈 조각이나 끝의 `/`가 있는 경로다",
  outside: "작업 사본 밖의 경로다",
  symlink: "심볼릭 링크이거나 심볼릭 링크를 거친다",
  caseMismatch: "실제 파일 이름과 대소문자(또는 유니코드 정규화)만 다르다",
  notDirectory: "중간 조각이 폴더가 아니다",
});

export const MODE = Object.freeze({ edit: "edit", shell: "shell" });

const folded = (name) => name.normalize("NFC").toLowerCase();

const realOrSelf = (path) => {
  try {
    return realpathSync.native(path);
  } catch {
    return path;
  }
};

export const TEMPORARY_ROOTS = [
  ...new Set([
    realOrSelf(tmpdir()),
    "/tmp",
    "/private/tmp",
    "/var/folders",
    "/private/var/folders",
  ]),
];

const entries = (directory) => {
  try {
    return { names: readdirSync(directory) };
  } catch (error) {
    return { code: error.code };
  }
};

const walk = (root, relative) => {
  let current = root;
  for (const segment of relative.split("/")) {
    const listed = entries(current);
    if (listed.code === "ENOTDIR") {
      return PATH_PROBLEM.notDirectory;
    }
    if (listed.names === undefined) {
      return undefined;
    }
    if (!listed.names.includes(segment)) {
      return listed.names.some((name) => folded(name) === folded(segment))
        ? PATH_PROBLEM.caseMismatch
        : undefined;
    }
    current = `${current}/${segment}`;
    if (lstatSync(current).isSymbolicLink()) {
      return PATH_PROBLEM.symlink;
    }
  }
  return undefined;
};

export const shapeProblem = (path) => {
  if (typeof path !== "string" || path === "") {
    return PATH_PROBLEM.missing;
  }
  if (!isAbsolute(path)) {
    return PATH_PROBLEM.relative;
  }
  const parts = path.split("/").slice(1);
  return parts.some((part) => part === "" || part === "." || part === "..")
    ? PATH_PROBLEM.unnormalized
    : undefined;
};

const rootOf = (path, worktrees) =>
  worktrees
    .filter((candidate) => path.startsWith(`${candidate}/`))
    .sort((left, right) => right.length - left.length)
    .at(0);

export const locate = (path, worktrees) => {
  const shape = shapeProblem(path);
  if (shape !== undefined) {
    return { problem: shape };
  }
  const root = rootOf(path, worktrees);
  if (root === undefined) {
    return { problem: PATH_PROBLEM.outside };
  }
  const relative = path.slice(root.length + 1);
  const problem = walk(root, relative);
  return problem === undefined
    ? { root, relative }
    : { problem, root, relative };
};

const homePattern = (pattern) =>
  pattern.startsWith("~/")
    ? `${homedir().slice(1)}/${pattern.slice(2)}`
    : pattern;

const homeMatches = (patterns, path) =>
  patterns.some((pattern) =>
    patternMatches(homePattern(pattern), path.slice(1)),
  );

const under = (path, roots) =>
  roots.some((root) => path === root || path.startsWith(`${root}/`));

const containsSegments = (segments, wanted) =>
  segments.some((_, start) =>
    wanted.every((piece, offset) => segments[start + offset] === folded(piece)),
  );

const nameMatches = (pattern, name) => {
  const escaped = folded(pattern)
    .split("*")
    .map((piece) => piece.replace(/[.+?^${}()|[\]\\]/gu, "\\$&"))
    .join(".*");
  return new RegExp(`^${escaped}$`, "u").test(folded(name));
};

const finding = (rule, message, gated = false) => ({ rule, gated, message });

export const fixedFindings = (relative, rules) => {
  const segments = relative.split("/").map(folded);
  const findings = rules.paths.forbiddenSegments
    .filter((wanted) => containsSegments(segments, wanted))
    .map((wanted) =>
      finding(
        "forbidden-path",
        `${wanted.join("/")}는 어떤 역할도 쓰지 않는다: ${relative}`,
      ),
    );
  const config = rules.paths.configFiles;
  const name = relative.split("/").at(-1);
  if (
    config.names.some((pattern) => nameMatches(pattern, name)) &&
    !config.allowed.some((pattern) => patternMatches(pattern, relative)) &&
    !config.exempt.some((pattern) => patternMatches(pattern, relative))
  ) {
    findings.push(
      finding(
        "misplaced-config",
        `설정 파일 이름(${name})은 정해진 위치에만 둔다. 하위 폴더의 설정은 검사 동작을 바꾼다 (ADR 0015, 0017): ${relative}`,
      ),
    );
  }
  return findings;
};

const linkCount = (path) => {
  try {
    const status = statSync(path);
    return status.isFile() ? status.nlink : 1;
  } catch {
    return 1;
  }
};

const adrFindings = (relative, project) => {
  const { directory } = project.rules.paths.adr;
  if (!relative.startsWith(`${directory}/`)) {
    return [];
  }
  const tracked = project.adrs ?? [];
  if (tracked.some((path) => folded(path) === folded(relative))) {
    return [
      finding(
        "protected-path",
        `git에 추적된 ADR은 고치지 않는다. 새 ADR은 새 번호로 쓴다: ${relative}`,
        true,
      ),
    ];
  }
  const number = /^(\d+)/u.exec(relative.slice(directory.length + 1));
  const taken =
    number !== null &&
    tracked.some(
      (path) =>
        /^(\d+)/u.exec(path.slice(directory.length + 1))?.[1] === number[1],
    );
  return taken
    ? [
        finding(
          "adr-number",
          `이미 있는 ADR 번호 ${number[1]}다. 새 번호를 쓴다: ${relative}`,
          true,
        ),
      ]
    : [];
};

const repositoryFindings = (relative, role, project) => {
  const { rules, allowlist } = project;
  if (allowlist.problem !== undefined) {
    return [
      finding(
        "allowlist-unreadable",
        `쓰기 허용 목록을 읽지 못했다: ${allowlist.problem}`,
      ),
    ];
  }
  const mainOnly = rules.paths.mainOnly.some((pattern) =>
    patternMatchesIgnoringCase(pattern, relative),
  );
  if (mainOnly) {
    return role === ROLE.main
      ? []
      : [
          finding(
            "main-only-path",
            `리뷰 기록·작업 목록·진행 기록은 메인만 쓴다: ${relative}`,
          ),
        ];
  }
  const guarded = [
    ...adrFindings(relative, project),
    ...(isWritable(allowlist, relative)
      ? []
      : [
          finding(
            "protected-path",
            `보호 경로다 (docs/harness.md 7.1, harness/agent-writable.txt 밖): ${relative}`,
            true,
          ),
        ]),
  ];
  if (guarded.length > 0) {
    return guarded;
  }
  if (
    role === ROLE.main &&
    !rules.paths.mainPatterns.some((pattern) =>
      patternMatches(pattern, relative),
    )
  ) {
    return [
      finding(
        "main-docs-only",
        `메인 에이전트는 ${rules.paths.mainPatterns.join(", ")}만 쓴다. 코드는 implementer가 쓴다: ${relative}`,
      ),
    ];
  }
  return [];
};

export const realTarget = (path) => {
  let existing = path;
  const rest = [];
  while (!existsSync(existing) && dirname(existing) !== existing) {
    rest.unshift(basename(existing));
    existing = dirname(existing);
  }
  return join(realOrSelf(existing), ...rest);
};

const hardLinked = (path) =>
  linkCount(path) > 1
    ? [
        finding(
          "hard-link",
          `하드 링크가 걸린 파일(링크 수 2 이상)에는 쓰지 않는다: ${path}`,
        ),
      ]
    : [];

export const writeFindings = (path, role, project, mode) => {
  const { rules } = project;
  const claudeFolder =
    mode === MODE.edit &&
    role === ROLE.main &&
    homeMatches(rules.paths.claudeFolders, path);
  if (!claudeFolder && homeMatches(rules.paths.protectedHome, path)) {
    return [
      finding(
        "protected-home",
        `git·Claude Code·Rust·npm 사용자 설정은 쓰지 않는다: ${path}`,
      ),
    ];
  }
  const located = locate(path, project.worktrees);
  if (located.root === undefined) {
    if (located.problem !== PATH_PROBLEM.outside) {
      return [finding("unsafe-path", `${located.problem}: ${path}`)];
    }
    const anywhere = fixedFindings(path.slice(1), rules).filter(
      (item) => item.rule === "forbidden-path",
    );
    if (anywhere.length > 0) {
      return anywhere;
    }
    const real = realTarget(path);
    if (real !== path) {
      return writeFindings(real, role, project, mode);
    }
    if (hardLinked(path).length > 0) {
      return hardLinked(path);
    }
    return mode === MODE.shell || claudeFolder || under(path, TEMPORARY_ROOTS)
      ? []
      : [finding("unsafe-path", `${PATH_PROBLEM.outside}: ${path}`)];
  }
  const artifact = rules.paths.artifacts.some((pattern) =>
    patternMatches(pattern, located.relative),
  );
  if (located.problem !== undefined && !(artifact && mode === MODE.shell)) {
    return [finding("unsafe-path", `${located.problem}: ${path}`)];
  }
  if (hardLinked(path).length > 0) {
    return hardLinked(path);
  }
  const fixed = fixedFindings(located.relative, rules);
  if (fixed.length > 0) {
    return fixed;
  }
  if (artifact) {
    return [];
  }
  if (role === ROLE.reviewer || role === ROLE.unknown) {
    return [
      finding(
        "role-without-writes",
        `리뷰어·비판자·정의 밖 서브에이전트는 저장소에 쓰지 않는다: ${located.relative}`,
      ),
    ];
  }
  return repositoryFindings(located.relative, role, project);
};
