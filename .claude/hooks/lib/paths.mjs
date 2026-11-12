import { lstatSync, readdirSync } from "node:fs";
import { isAbsolute } from "node:path";
import { isWritable, patternMatches } from "./allowlist.mjs";
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

const folded = (name) => name.normalize("NFC").toLowerCase();

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

export const locate = (path, worktrees) => {
  if (typeof path !== "string" || path === "") {
    return { problem: PATH_PROBLEM.missing };
  }
  if (!isAbsolute(path)) {
    return { problem: PATH_PROBLEM.relative };
  }
  const parts = path.split("/").slice(1);
  if (parts.some((part) => part === "" || part === "." || part === "..")) {
    return { problem: PATH_PROBLEM.unnormalized };
  }
  const root = worktrees
    .filter((candidate) => path.startsWith(`${candidate}/`))
    .sort((left, right) => right.length - left.length)
    .at(0);
  if (root === undefined) {
    return { problem: PATH_PROBLEM.outside };
  }
  const relative = path.slice(root.length + 1);
  const problem = walk(root, relative);
  return problem === undefined ? { root, relative } : { problem, root };
};

const containsSegments = (segments, wanted) =>
  segments.some((_, start) =>
    wanted.every((piece, offset) => segments[start + offset] === folded(piece)),
  );

export const fixedFindings = (relative, rules) => {
  const segments = relative.split("/").map(folded);
  const findings = rules.paths.forbiddenSegments
    .filter((wanted) => containsSegments(segments, wanted))
    .map((wanted) => ({
      rule: "forbidden-path",
      gated: false,
      message: `${wanted.join("/")}는 어떤 역할도 쓰지 않는다: ${relative}`,
    }));
  const name = segments.at(-1);
  if (
    segments.length > 1 &&
    rules.paths.rootOnlyNames.some((only) => folded(only) === name)
  ) {
    findings.push({
      rule: "nested-root-only-file",
      gated: false,
      message: `${name}는 저장소 루트에만 둔다 (ADR 0015): ${relative}`,
    });
  }
  return findings;
};

export const protectedFinding = (relative, allowlist) => {
  if (allowlist.problem !== undefined) {
    return [
      {
        rule: "allowlist-unreadable",
        gated: false,
        message: `쓰기 허용 목록을 읽지 못했다: ${allowlist.problem}`,
      },
    ];
  }
  return isWritable(allowlist, relative)
    ? []
    : [
        {
          rule: "protected-path",
          gated: true,
          message: `보호 경로다 (docs/harness.md 7.1, harness/agent-writable.txt 밖): ${relative}`,
        },
      ];
};

export const roleFindings = (relative, role, allowlist, rules) => {
  const guarded = protectedFinding(relative, allowlist);
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
      {
        rule: "main-docs-only",
        gated: false,
        message: `메인 에이전트는 ${rules.paths.mainPatterns.join(", ")}만 편집한다. 코드는 implementer가 쓴다: ${relative}`,
      },
    ];
  }
  return [];
};
