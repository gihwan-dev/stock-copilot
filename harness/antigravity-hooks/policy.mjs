import { lstatSync, readFileSync, readlinkSync, realpathSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { basename, dirname, isAbsolute, join, resolve } from "node:path";
import {
  patternMatches,
  patternMatchesIgnoringCase,
} from "../../.claude/hooks/lib/allowlist.mjs";
import { readPolicy } from "../sandbox/fragment.mjs";

export const ANTIGRAVITY_DENY_READ = Object.freeze([
  "~/.gemini",
  "~/.antigravity",
  "~/Library/Application Support/Antigravity",
  "~/.zsh_history",
  "~/.bash_history",
  "~/.sh_history",
  "~/.history",
  "~/.zhistory",
]);

export const ANTIGRAVITY_PROTECTED_HOME = Object.freeze([
  "~/.gemini/**",
  "~/.antigravity/**",
  "~/Library/Application Support/Antigravity/**",
  "~/.zshrc",
  "~/.zprofile",
  "~/.zshenv",
  "~/.bashrc",
  "~/.bash_profile",
  "~/.profile",
]);

export const ANTIGRAVITY_HOOK_CONFIG = ".agents/hooks.json";

export const ANTIGRAVITY_CONFIG_NAMES = Object.freeze([
  "*.agents/hooks.json",
  "*.agent/hooks.json",
  "*_agents/hooks.json",
  "*_agent/hooks.json",
  "GEMINI.md",
]);

export const ANTIGRAVITY_CONFIG_ALLOWED = Object.freeze([
  ANTIGRAVITY_HOOK_CONFIG,
]);

export const MEMORY_BASENAMES = Object.freeze(["memory.md"]);

export const NETWORK_CLIENTS = Object.freeze([
  "curl",
  "wget",
  "http",
  "xh",
  "nc",
  "ncat",
  "socat",
  "telnet",
  "openssl",
  "ftp",
  "sftp",
  "ssh",
  "scp",
  "slogin",
  "rsh",
  "rcp",
  "tftp",
  "aria2c",
  "lwp-request",
  "fetch",
  "lynx",
  "w3m",
  "links",
  "whois",
  "dig",
  "nslookup",
  "host",
  "ping",
  "traceroute",
]);

export const TOSS_LIVE_BINARIES = Object.freeze([
  "toss-smoke",
  "src-tauri",
  "broker-toss-trade",
]);

export const LOOPBACK_HOSTS = Object.freeze([
  "localhost",
  "127.0.0.1",
  "::1",
  "[::1]",
]);

export const KEYCHAIN_DENIED_SUBCOMMANDS = Object.freeze([
  "find-generic-password",
  "find-internet-password",
  "dump-keychain",
  "export",
  "find-certificate",
  "find-identity",
  "find-key",
  "show-keychain-info",
  "list-keychains",
  "unlock-keychain",
  "dump-trust-settings",
  "cms",
  "import",
]);

const folded = (text) => text.normalize("NFC").toLowerCase();

const hasGlob = (text) => /[*?[\]{}]/u.test(text);

const globSegmentRegex = (segment) => {
  let pattern = "^";
  let index = 0;
  const source = folded(segment);
  while (index < source.length) {
    const ch = source[index];
    if (ch === "*") {
      pattern += "[^/]*";
      index += 1;
    } else if (ch === "?") {
      pattern += "[^/]";
      index += 1;
    } else if (ch === "[") {
      const close = source.indexOf("]", index + 1);
      if (close !== -1) {
        const inner = source.slice(index + 1, close);
        const negated = inner.startsWith("!") || inner.startsWith("^");
        const body = (negated ? inner.slice(1) : inner).replace(
          /[\\]/gu,
          "\\\\",
        );
        pattern += `[${negated ? "^" : ""}${body}]`;
        index = close + 1;
      } else {
        pattern += "\\[";
        index += 1;
      }
    } else if (ch === "{") {
      const close = source.indexOf("}", index + 1);
      if (close !== -1) {
        pattern += "[^/]*";
        index = close + 1;
      } else {
        pattern += "\\{";
        index += 1;
      }
    } else {
      pattern += ch.replace(/[.+^$()|\\]/gu, "\\$&");
      index += 1;
    }
  }
  return new RegExp(`${pattern}$`, "u");
};

const segmentMatchesGlob = (candidateSeg, literalSeg) => {
  if (candidateSeg === "**") return true;
  if (!hasGlob(candidateSeg))
    return folded(candidateSeg) === folded(literalSeg);
  try {
    return globSegmentRegex(candidateSeg).test(folded(literalSeg));
  } catch {
    return true;
  }
};

const realOrSelf = (path) => {
  try {
    return realpathSync.native(path);
  } catch {
    return path;
  }
};

export const tossHostFromRules = (root) => {
  const text = readFileSync(
    join(root, "harness/arch-check/rules.toml"),
    "utf8",
  );
  const section = /\[source\.toss-host\]([\s\S]*?)(?:\n\[|$)/u.exec(text);
  const needle =
    section === null ? null : /\bneedle\s*=\s*"([^"]+)"/u.exec(section[1]);
  if (needle === null || needle[1].trim() === "") {
    throw new Error(
      "harness/arch-check/rules.toml에서 토스 호스트를 읽지 못했다",
    );
  }
  return needle[1].trim().toLowerCase();
};

export const loadAntigravityPolicy = (root, project) => {
  const sandbox = readPolicy(root);
  const denyRead = sandbox?.base?.filesystem?.denyRead;
  const allowedDomains = sandbox?.base?.network?.allowedDomains;
  const protectedHome = project?.rules?.paths?.protectedHome;
  if (
    !Array.isArray(denyRead) ||
    !Array.isArray(allowedDomains) ||
    !Array.isArray(protectedHome)
  ) {
    throw new Error("공유 정책 파일(policy.json, rules.json)을 읽지 못했다");
  }
  return {
    denyRead: [
      ...new Set([...denyRead, ...protectedHome, ...ANTIGRAVITY_DENY_READ]),
    ],
    protectedHome: [
      ...new Set([...protectedHome, ...ANTIGRAVITY_PROTECTED_HOME]),
    ],
    allowedDomains: allowedDomains.map((domain) => domain.toLowerCase()),
    tossHost: tossHostFromRules(root),
  };
};

export const expandHome = (raw, home = homedir()) => {
  if (typeof raw !== "string" || raw.length === 0) {
    return { problem: "빈 경로 문자열이다" };
  }
  const normalizedHome = home.replace(/\/+$/u, "");
  if (raw === "~") return { path: normalizedHome };
  if (raw.startsWith("~/"))
    return { path: `${normalizedHome}/${raw.slice(2)}` };
  if (raw.startsWith("~")) return { unknownHome: true };
  if (raw === "$HOME" || raw === "${HOME}") return { path: normalizedHome };
  if (raw.startsWith("$HOME/")) {
    return { path: `${normalizedHome}/${raw.slice("$HOME/".length)}` };
  }
  if (raw.startsWith("${HOME}/")) {
    return { path: `${normalizedHome}/${raw.slice("${HOME}/".length)}` };
  }
  return { path: raw };
};

const patternRelative = (pattern) => {
  const trimmed = pattern.replace(/\/+$/u, "");
  return trimmed.startsWith("~/") ? trimmed.slice(2) : undefined;
};

const matchesHomeEntry = (pattern, absolutePath, homeRoot) => {
  const relativePattern = patternRelative(pattern);
  if (relativePattern === undefined) return false;
  const prefix = `${homeRoot}/`;
  if (!absolutePath.startsWith(prefix)) return false;
  const relative = absolutePath.slice(prefix.length);
  if (patternMatchesIgnoringCase(relativePattern, relative)) return true;
  if (relativePattern.endsWith("/**")) {
    return patternMatchesIgnoringCase(relativePattern.slice(0, -3), relative);
  }
  return patternMatchesIgnoringCase(`${relativePattern}/**`, relative);
};

const matchesGlobUnderHome = (pattern, relativeCandidate) => {
  const relativePattern = patternRelative(pattern);
  if (relativePattern === undefined) return false;
  const patternSegs = relativePattern
    .replace(/\/\*\*$/u, "")
    .split("/")
    .filter(Boolean);
  const candSegs = relativeCandidate.split("/").filter(Boolean);
  const len = Math.min(patternSegs.length, candSegs.length);
  if (len === 0) return false;
  for (let index = 0; index < len; index += 1) {
    const cSeg = candSegs[index];
    const pSeg = patternSegs[index];
    if (cSeg === "**" || pSeg === "**") return true;
    if (!segmentMatchesGlob(cSeg, pSeg) && !segmentMatchesGlob(pSeg, cSeg)) {
      return false;
    }
  }
  return true;
};

const MISSING_PATH_CODES = new Set(["ENOENT", "ENOTDIR", "ENAMETOOLONG"]);

export const resolveLinkChain = (lexicalPath) => {
  const visited = new Set([lexicalPath]);
  const results = [lexicalPath];
  const inspect = (path, depth) => {
    if (depth > 16) {
      throw new Error(`심볼릭 링크 중첩 깊이가 상한을 넘었다: ${lexicalPath}`);
    }
    const parts = path.split("/").slice(1);
    let current = "";
    for (let index = 0; index < parts.length; index += 1) {
      current = `${current}/${parts[index]}`;
      let stat;
      try {
        stat = lstatSync(current);
      } catch (error) {
        if (MISSING_PATH_CODES.has(error?.code)) return;
        throw new Error(
          `경로 상태를 확인할 수 없다(${error?.code ?? "unknown"}): ${current}`,
          { cause: error },
        );
      }
      if (stat.isSymbolicLink()) {
        let target;
        try {
          target = readlinkSync(current);
        } catch (error) {
          throw new Error(
            `심볼릭 링크 대상을 읽을 수 없다(${error?.code ?? "unknown"}): ${current}`,
            { cause: error },
          );
        }
        const resolved = isAbsolute(target)
          ? resolve(target)
          : resolve(dirname(current), target);
        const tail = parts.slice(index + 1);
        const joined = tail.length === 0 ? resolved : join(resolved, ...tail);
        if (!visited.has(joined)) {
          visited.add(joined);
          results.push(joined);
          inspect(joined, depth + 1);
        }
        return;
      }
    }
  };
  inspect(lexicalPath, 0);
  const nativeReal = realOrSelf(lexicalPath);
  if (!visited.has(nativeReal)) {
    visited.add(nativeReal);
    results.push(nativeReal);
  }
  return results;
};

const within = (candidate, root) =>
  typeof root === "string" &&
  root.length > 0 &&
  (candidate === root || candidate.startsWith(`${root}/`));

const matchesHomePrefixGlob = (candidate, homeRoot) => {
  const homeSegs = homeRoot.split("/").filter(Boolean);
  const candSegs = candidate.split("/").filter(Boolean);
  if (candSegs.length === 0) return { kind: "ancestor" };
  const starIdx = candSegs.indexOf("**");
  if (
    starIdx !== -1 &&
    starIdx <= homeSegs.length &&
    candSegs
      .slice(0, starIdx)
      .every((seg, i) => segmentMatchesGlob(seg, homeSegs[i]))
  ) {
    return { kind: "home-glob" };
  }
  if (
    candSegs.length <= homeSegs.length &&
    candSegs.every((seg, i) => segmentMatchesGlob(seg, homeSegs[i]))
  ) {
    return { kind: candSegs.length === homeSegs.length ? "home" : "ancestor" };
  }
  if (
    candSegs.length > homeSegs.length &&
    candSegs.slice(0, homeSegs.length).some(hasGlob) &&
    candSegs
      .slice(0, homeSegs.length)
      .every((seg, i) => segmentMatchesGlob(seg, homeSegs[i]))
  ) {
    return { kind: "home-glob" };
  }
  return undefined;
};

export const isDeniedReadPath = (
  candidate,
  policy,
  ownedWorktree,
  home = homedir(),
) => {
  const inWorktree =
    within(candidate, ownedWorktree) ||
    within(candidate, realOrSelf(ownedWorktree ?? ""));
  const homes = [
    ...new Set([
      home.replace(/\/+$/u, ""),
      realOrSelf(home).replace(/\/+$/u, ""),
    ]),
  ];
  for (const homeRoot of homes) {
    if (candidate === homeRoot) {
      return {
        rule: "protected-home-read",
        gated: false,
        message: `사용자 홈 폴더 전체를 직접 읽거나 탐색하지 않는다: ${candidate}`,
      };
    }
    if (!inWorktree) {
      const prefixHit = matchesHomePrefixGlob(candidate, homeRoot);
      if (prefixHit !== undefined) {
        return {
          rule: "protected-home-read",
          gated: false,
          message:
            prefixHit.kind === "home-glob"
              ? `사용자 홈 폴더 이름의 와일드카드 경로는 읽지 않는다: ${candidate}`
              : `사용자 홈 또는 상위 폴더를 직접 읽거나 탐색하지 않는다: ${candidate}`,
        };
      }
    }
    const homePrefix = `${homeRoot}/`;
    if (candidate.startsWith(homePrefix) && !inWorktree) {
      const relative = candidate.slice(homePrefix.length);
      const firstSegment = relative.split("/")[0];
      if (hasGlob(firstSegment)) {
        return {
          rule: "protected-home-read",
          gated: false,
          message: `사용자 홈 아래 와일드카드 경로는 읽지 않는다: ${candidate}`,
        };
      }
      if (hasGlob(relative)) {
        for (const pattern of policy.denyRead) {
          if (matchesGlobUnderHome(pattern, relative)) {
            return {
              rule: "protected-home-read",
              gated: false,
              message: `사용자 홈 보호 경로와 겹치는 와일드카드(${pattern})는 읽지 않는다: ${candidate}`,
            };
          }
        }
      }
    }
    for (const pattern of policy.denyRead) {
      if (matchesHomeEntry(pattern, candidate, homeRoot)) {
        return {
          rule: "protected-home-read",
          gated: false,
          message: `사용자 홈의 설정·자격 증명·하네스 상태는 읽지 않는다 (${pattern}): ${candidate}`,
        };
      }
    }
  }
  if (MEMORY_BASENAMES.includes(folded(basename(candidate))) && !inWorktree) {
    return {
      rule: "protected-memory-read",
      gated: false,
      message: `작업 사본 밖의 메모리 파일은 읽지 않는다: ${candidate}`,
    };
  }
  return undefined;
};

export const readTargetFindings = (
  rawPath,
  cwd,
  policy,
  ownedWorktree,
  home = homedir(),
) => {
  const expanded = expandHome(rawPath, home);
  if (expanded.unknownHome) {
    return [
      {
        rule: "protected-home-read",
        gated: false,
        message: `다른 사용자 홈 경로(${rawPath})는 읽지 않는다`,
      },
    ];
  }
  if (expanded.problem !== undefined) {
    return [
      {
        rule: "unsafe-read-path",
        gated: false,
        message: `${expanded.problem}: ${String(rawPath)}`,
      },
    ];
  }
  const lexical = isAbsolute(expanded.path)
    ? resolve(expanded.path)
    : typeof cwd === "string" && cwd.length > 0
      ? resolve(cwd, expanded.path)
      : undefined;
  if (lexical === undefined) {
    return [
      {
        rule: "unsafe-read-path",
        gated: false,
        message: `현재 폴더를 알 수 없어 읽기 경로를 풀 수 없다: ${rawPath}`,
      },
    ];
  }
  const findings = [];
  for (const candidate of resolveLinkChain(lexical)) {
    const hit = isDeniedReadPath(candidate, policy, ownedWorktree, home);
    if (hit !== undefined) {
      findings.push(hit);
      break;
    }
  }
  return findings;
};

export const homeWriteFindings = (rawPath, policy, home = homedir()) => {
  const expanded = expandHome(rawPath, home);
  if (expanded.unknownHome) {
    return [
      {
        rule: "protected-home",
        gated: false,
        message: `다른 사용자 홈 경로(${rawPath})에는 쓰지 않는다`,
      },
    ];
  }
  if (expanded.problem !== undefined || !isAbsolute(expanded.path)) return [];
  const homes = [
    ...new Set([
      home.replace(/\/+$/u, ""),
      realOrSelf(home).replace(/\/+$/u, ""),
    ]),
  ];
  for (const candidate of resolveLinkChain(resolve(expanded.path))) {
    for (const homeRoot of homes) {
      const prefixHit = matchesHomePrefixGlob(candidate, homeRoot);
      if (prefixHit !== undefined && prefixHit.kind === "home-glob") {
        return [
          {
            rule: "protected-home",
            gated: false,
            message: `사용자 홈 폴더 이름에 와일드카드를 포함한 경로에는 쓰지 않는다: ${candidate}`,
          },
        ];
      }
      for (const pattern of policy.protectedHome) {
        if (matchesHomeEntry(pattern, candidate, homeRoot)) {
          return [
            {
              rule: "protected-home",
              gated: false,
              message: `사용자 홈 설정(${pattern})에는 쓰지 않는다: ${candidate}`,
            },
          ];
        }
      }
    }
  }
  return [];
};

const DEV_WRITABLES = ["/dev/null", "/dev/stdout", "/dev/stderr"];

export const outsideWorktreeShellWriteFindings = (
  candidate,
  ownedWorktree,
  home = homedir(),
) => {
  if (
    within(candidate, ownedWorktree) ||
    within(candidate, realOrSelf(ownedWorktree ?? ""))
  ) {
    return [];
  }
  if (DEV_WRITABLES.includes(candidate) || candidate.startsWith("/dev/fd/")) {
    return [];
  }
  const homes = [
    ...new Set([
      home.replace(/\/+$/u, ""),
      realOrSelf(home).replace(/\/+$/u, ""),
    ]),
  ];
  if (homes.some((homeRoot) => within(candidate, homeRoot))) {
    return [
      {
        rule: "outside-worktree-write",
        gated: false,
        message: `작업 사본 밖의 사용자 홈 경로에는 셸로 쓰지 않는다: ${candidate}`,
      },
    ];
  }
  const tempRoots = [
    ...new Set([
      "/tmp",
      "/private/tmp",
      "/var/folders",
      "/private/var/folders",
      tmpdir().replace(/\/+$/u, ""),
      realOrSelf(tmpdir()).replace(/\/+$/u, ""),
    ]),
  ];
  if (tempRoots.some((root) => within(candidate, root))) {
    return [];
  }
  return [
    {
      rule: "outside-worktree-write",
      gated: false,
      message: `작업 사본과 임시 폴더 밖 경로에는 셸로 쓰지 않는다: ${candidate}`,
    },
  ];
};

const nameMatches = (pattern, target) => {
  const escaped = folded(pattern)
    .split("*")
    .map((piece) => piece.replace(/[.+?^${}()|[\]\\]/gu, "\\$&"))
    .join(".*");
  return new RegExp(`^${escaped}$`, "u").test(folded(target));
};

export const antigravityPathFindings = (relative) => {
  const findings = [];
  const lower = folded(relative).replace(/\/+$/u, "");
  if (
    lower === ".agents" ||
    lower === folded(ANTIGRAVITY_HOOK_CONFIG) ||
    lower.startsWith(".agents/")
  ) {
    findings.push({
      rule: "antigravity-hook-config",
      gated: false,
      message: `Antigravity 훅 설정(${relative})은 에이전트가 직접 수정하지 않는다`,
    });
  }
  const name = lower.split("/").at(-1) ?? lower;
  if (
    ANTIGRAVITY_CONFIG_NAMES.some((pattern) =>
      nameMatches(pattern, pattern.includes("/") ? lower : name),
    ) &&
    !ANTIGRAVITY_CONFIG_ALLOWED.some((allowed) =>
      patternMatches(allowed, relative),
    )
  ) {
    findings.push({
      rule: "misplaced-config",
      gated: false,
      message: `Antigravity 설정 파일(${relative})은 허용된 위치에만 둔다`,
    });
  }
  return findings;
};
