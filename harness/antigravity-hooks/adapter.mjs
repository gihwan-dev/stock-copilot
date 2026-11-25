import { readFileSync, realpathSync, statSync } from "node:fs";
import { homedir } from "node:os";
import { basename, isAbsolute, join, resolve } from "node:path";
import { reportProblems } from "../../.claude/hooks/lib/agent-guard.mjs";
import {
  DEFINITIONS,
  WRITERS,
  bashFindings,
  unwrap,
} from "../../.claude/hooks/lib/bash-guard.mjs";
import {
  parseBrief,
  reviewTargetCommit,
} from "../../.claude/hooks/lib/brief.mjs";
import { editFindings } from "../../.claude/hooks/lib/edit-guard.mjs";
import {
  headOf,
  isAncestor,
  resolveCommit,
} from "../../.claude/hooks/lib/git-facts.mjs";
import { delegateTargetFindings } from "../../.claude/hooks/lib/orchestration.mjs";
import {
  locate,
  realTarget,
  shapeProblem,
} from "../../.claude/hooks/lib/paths.mjs";
import { LOCK, git } from "../../.claude/hooks/lib/project.mjs";
import { blockedExit } from "./staged-sync.mjs";
import { ROLE } from "../../.claude/hooks/lib/roles.mjs";
import { parseScript } from "../../.claude/hooks/lib/shell.mjs";
import { DECISION, decide } from "../../.claude/hooks/lib/verdict.mjs";
import {
  patchPaths,
  shellPatches,
  toolDirectory,
} from "../codex-hooks/adapter.mjs";
import {
  LOOPBACK_HOSTS,
  NETWORK_CLIENTS,
  TOSS_LIVE_BINARIES,
  antigravityPathFindings,
  expandHome,
  homeWriteFindings,
  loadAntigravityPolicy,
  outsideWorktreeShellWriteFindings,
  readTargetFindings,
} from "./policy.mjs";

const STOP_GUIDANCE =
  "하네스가 막은 것이다. 같은 일을 다른 방법으로 하려 하지 말고 멈춘 뒤 이유를 보고하라 (docs/harness.md 8장).";

export const WRITE_TOOLS = Object.freeze([
  "write_file",
  "edit_file",
  "write_to_file",
  "replace_file_content",
  "multi_replace_file_content",
  "Edit",
  "Write",
  "NotebookEdit",
]);

export const READ_TOOLS = Object.freeze([
  "read_file",
  "view_file",
  "view_file_outline",
  "view_code_item",
  "list_dir",
  "find_by_name",
  "grep_search",
  "Read",
  "Glob",
  "Grep",
]);

export const SHELL_TOOLS = Object.freeze([
  "run_command",
  "Bash",
  "exec_command",
  "shell_command",
  "shell",
]);

export const SAFE_REVIEWER_TOOLS = Object.freeze([
  "list_resources",
  "read_resource",
]);

export const SAFE_IMPLEMENTER_TOOLS = Object.freeze([
  ...SAFE_REVIEWER_TOOLS,
  "ask_question",
  "mcp_t3-code_orchestrator_capabilities",
  "mcp_t3-code_task_status",
  "mcp_t3-code_task_cancel",
  "mcp_t3-code_t3_worktree_status",
  "mcp_t3-code_t3_worktree_list",
  "mcp_t3-code_t3_environment_read",
  "mcp_t3-code_t3_project_list",
  "mcp_t3-code_t3_project_read",
  "mcp_t3-code_t3_thread_list",
  "mcp_t3-code_t3_thread_read",
  "mcp_t3-code_t3_thread_wait",
  "mcp_t3-code_t3_thread_search",
  "mcp_t3-code_list_issues",
  "mcp_t3-code_read_issue",
  "mcp_t3-code_list_thread_issues",
  "mcp_t3-code_pull_request_read",
  "mcp_t3-code_pull_request_list",
  "mcp_t3-code_list_thread_pull_requests",
  "mcp_t3-code_ci_logs",
  "mcp_t3-code_t3_preview_list",
  "mcp_t3-code_preview_status",
]);

const REVIEW_RECORD_PATTERN = /^docs\/reviews\/[A-Z]+-\d+\.md$/u;

const REVIEWER_DENIED_GIT_SUBCOMMANDS = new Set([
  "add",
  "commit",
  "rm",
  "mv",
  "clean",
  "restore",
  "stash",
  "reset",
  "merge",
  "tag",
  "notes",
  "init",
  "clone",
  "worktree",
  "fetch",
  "submodule",
  "sparse-checkout",
  "bisect",
  "gc",
  "prune",
  "repack",
  "pack-refs",
  "update-index",
  "read-tree",
  "write-tree",
  "checkout-index",
  "format-patch",
  "bundle",
  "archive",
]);

const FILE_READ_COMMANDS = new Set([
  "cat",
  "head",
  "tail",
  "less",
  "more",
  "grep",
  "egrep",
  "fgrep",
  "rg",
  "find",
  "ls",
  "stat",
  "file",
  "diff",
  "cmp",
  "jq",
  "base64",
  "xxd",
  "od",
  "hexdump",
  "strings",
  "wc",
  "sort",
  "cut",
  "awk",
  "gawk",
  "mawk",
  "nawk",
  "sed",
  "tr",
  "nl",
  "tac",
  "rev",
  "du",
  "tree",
  "shasum",
  "sha256sum",
  "md5",
  "ssh-keygen",
  "openssl",
  "plutil",
  "defaults",
  "sqlite3",
  "tar",
  "bsdtar",
  "gtar",
  "gnutar",
  "cpio",
  "pax",
  "unzip",
  "zip",
  "gzip",
  "bzip2",
  "xz",
]);

const GIT_NETWORK_SUBCOMMANDS = new Set([
  "ls-remote",
  "clone",
  "fetch",
  "pull",
  "push",
  "remote",
  "submodule",
]);

const ARCHIVE_EXTRACT_ONLY = new Set(["unzip", "unar", "unrar", "ditto"]);

const TAR_COMMANDS = new Set(["tar", "bsdtar", "gtar", "gnutar"]);

const SQLITE_META_PATTERN =
  /^\.(?:shell|system|output|once|load|read|excel|save|backup|restore|import|clone)\b/iu;

const REVIEWER_READONLY_BRANCH_FLAGS = new Set([
  "--show-current",
  "-l",
  "--list",
  "-a",
  "--all",
  "-r",
  "--remotes",
  "-v",
  "-vv",
  "--verbose",
  "--contains",
  "--merged",
  "--no-merged",
  "--format",
]);

const GIT_VALUED_GLOBALS = new Set([
  "-C",
  "-c",
  "--git-dir",
  "--work-tree",
  "--namespace",
  "--config-env",
  "--super-prefix",
  "--attr-source",
]);

const PRELOCK_GENERATORS = new Set([
  "harness/claude-hooks/hashes.mjs",
  "harness/sandbox/fragment.mjs",
]);

const LOCKED_CONFIG_GENERATORS = new Set([
  "harness/antigravity-hooks/config.mjs",
  "harness/codex-hooks/config.mjs",
]);

const HTTP_CLIENTS = new Set([
  "curl",
  "wget",
  "http",
  "xh",
  "aria2c",
  "lwp-request",
  "fetch",
  "lynx",
  "w3m",
  "links",
]);

const RAW_REMOTE_CLIENTS = new Set([
  "nc",
  "ncat",
  "socat",
  "telnet",
  "ssh",
  "scp",
  "sftp",
  "ftp",
  "slogin",
  "rsh",
  "rcp",
  "tftp",
  "whois",
  "dig",
  "nslookup",
  "host",
  "ping",
  "traceroute",
]);

const CURL_DENIED_FLAGS = new Set([
  "-n",
  "--netrc",
  "--netrc-optional",
  "--netrc-file",
  "-K",
  "--config",
  "-x",
  "--proxy",
  "--preproxy",
  "--socks4",
  "--socks4a",
  "--socks5",
  "--socks5-hostname",
  "--resolve",
  "--connect-to",
  "--unix-socket",
  "--abstract-unix-socket",
]);

const WGET_DENIED_FLAGS = new Set([
  "-i",
  "--input-file",
  "--config",
  "-e",
  "--execute",
  "--netrc-file",
  "--base",
  "--post-file",
  "--body-file",
  "--load-cookies",
  "--save-cookies",
  "--ca-certificate",
  "--certificate",
  "--private-key",
]);

const CURL_SHORT_VALUED = new Set([
  "X",
  "H",
  "d",
  "F",
  "o",
  "A",
  "u",
  "b",
  "c",
  "e",
  "m",
  "w",
  "T",
  "E",
  "K",
  "x",
  "U",
  "r",
  "y",
  "Y",
  "z",
  "D",
  "C",
  "Q",
  "t",
]);

const CURL_SHORT_DENIED_READ = new Set(["n"]);
const CURL_SHORT_DENIED_NET = new Set(["K", "x"]);

const WGET_SHORT_VALUED = new Set([
  "O",
  "o",
  "a",
  "i",
  "e",
  "P",
  "U",
  "B",
  "Q",
  "t",
  "T",
  "w",
  "l",
  "A",
  "R",
  "D",
  "X",
  "I",
]);

const WGET_SHORT_DENIED = new Set(["i", "e"]);

const CURL_VALUED_FLAGS = new Set([
  "-X",
  "--request",
  "-H",
  "--header",
  "-d",
  "--data",
  "--data-raw",
  "--data-binary",
  "--data-urlencode",
  "-F",
  "--form",
  "-o",
  "--output",
  "-A",
  "--user-agent",
  "-u",
  "--user",
  "-b",
  "--cookie",
  "-c",
  "--cookie-jar",
  "-e",
  "--referer",
  "-m",
  "--max-time",
  "--connect-timeout",
  "-w",
  "--write-out",
  "-T",
  "--upload-file",
]);

const CARGO_VALUED_GLOBALS = new Set([
  "-C",
  "--manifest-path",
  "--config",
  "--color",
  "-Z",
  "--target-dir",
  "--explain",
]);

const realOrSelf = (path) => {
  try {
    return realpathSync.native(path);
  } catch {
    return path;
  }
};

const plainObject = (value) =>
  value !== null && typeof value === "object" && !Array.isArray(value);

const finding = (rule, message, gated = false) => ({ rule, gated, message });

export const denyOutput = (findings) => ({
  decision: "deny",
  reason: `${findings.map((item) => `[${item.rule}] ${item.message}`).join("\n")}\n${STOP_GUIDANCE}`,
});

export const refusal = (message) =>
  denyOutput([finding("antigravity-input", message)]);

const currentBranch = (root) =>
  git(root, ["symbolic-ref", "--quiet", "--short", "HEAD"])?.trim();

const isImplementerModelName = (modelName) =>
  typeof modelName === "string" &&
  /^gemini-4-neon-xhigh$/iu.test(modelName.trim());

const isDelegatedThreadId = (threadId) =>
  typeof threadId === "string" &&
  /^thread:delegated-task:/iu.test(threadId.trim());

const isDelegatedWorktreePath = (root) =>
  /^t3-[0-9a-f]{8}-/iu.test(basename(root));

const isNonImplementerBranch = (branch) =>
  branch === undefined ||
  branch === "" ||
  ["master", "main"].includes(branch.toLowerCase()) ||
  /^(?:merge|review|critic|t3)\//iu.test(branch);

export const actorInput = (
  input,
  project,
  sessionCwd = process.cwd(),
  extraFacts = {},
) => {
  if (!plainObject(input)) {
    throw new Error("훅 입력 JSON이 객체가 아니다");
  }
  const located = locate(
    resolve(sessionCwd, ".antigravity-role-probe"),
    project.worktrees,
  );
  if (located.root === undefined || located.problem !== undefined) {
    throw new Error("등록된 세션 작업 사본이 아니다");
  }
  const explicitType = input.agent_type;
  if (
    explicitType !== undefined &&
    !["implementer", "adversarial-reviewer", "review-critic"].includes(
      explicitType,
    )
  ) {
    return {
      root: located.root,
      role: ROLE.unknown,
      branch: undefined,
      actor: {
        agent_type: String(explicitType),
        agent_id: "antigravity-unknown",
      },
    };
  }
  if (
    explicitType === "adversarial-reviewer" ||
    explicitType === "review-critic"
  ) {
    return {
      root: located.root,
      role: ROLE.reviewer,
      branch: undefined,
      actor: { agent_type: explicitType, agent_id: "antigravity-reviewer" },
    };
  }
  if (located.root === project.root) {
    return {
      root: located.root,
      role: ROLE.reviewer,
      branch: undefined,
      actor: {
        agent_type: "review-critic",
        agent_id: "antigravity-root-readonly",
      },
    };
  }
  const branch = Object.hasOwn(extraFacts, "branch")
    ? extraFacts.branch
    : currentBranch(located.root);
  const threadId = Object.hasOwn(extraFacts, "threadId")
    ? extraFacts.threadId
    : process.env.T3CODE_THREAD_ID;
  if (isDelegatedThreadId(threadId)) {
    return {
      root: located.root,
      role: ROLE.reviewer,
      branch,
      actor: {
        agent_type: "review-critic",
        agent_id: "antigravity-delegated-child",
      },
    };
  }
  const reviewer =
    isNonImplementerBranch(branch) ||
    isDelegatedWorktreePath(located.root) ||
    !isImplementerModelName(input.modelName) ||
    extraFacts.t3Role === "review" ||
    extraFacts.t3Role === "research";
  if (reviewer) {
    return {
      root: located.root,
      role: ROLE.reviewer,
      branch,
      actor: {
        agent_type: "review-critic",
        agent_id: "antigravity-reviewer",
      },
    };
  }
  return {
    root: located.root,
    role: ROLE.implementer,
    branch,
    actor: {
      agent_type: "implementer",
      agent_id: "antigravity-worktree",
    },
  };
};

const writePathsFromArgs = (toolName, args) => {
  if (!plainObject(args)) {
    return { problem: "도구 인자가 객체가 아니다" };
  }
  const explicit = [
    ...(toolName === "NotebookEdit" ? [args.notebook_path] : []),
    args.Path,
    args.TargetFile,
    args.file_path,
    args.path,
    args.AbsolutePath,
    args.File,
  ].filter((value) => value !== undefined);
  const extra = Object.entries(args)
    .filter(
      ([key, value]) =>
        typeof value === "string" &&
        /(?:path|file|dir|directory|root)$/iu.test(key) &&
        !explicit.includes(value),
    )
    .map(([, value]) => value);
  const candidates = [...new Set([...explicit, ...extra])];
  if (candidates.length === 0) {
    return { problem: `${toolName}의 대상 경로가 없다` };
  }
  if (candidates.some((value) => typeof value !== "string" || value === "")) {
    return { problem: `${toolName}의 대상 경로가 문자열이 아니다` };
  }
  return { paths: candidates };
};

const readPathsFromArgs = (toolName, args, fallbackCwd) => {
  if (!plainObject(args)) {
    return { problem: "도구 인자가 객체가 아니다" };
  }
  const explicit = [
    args.Path,
    args.AbsolutePath,
    args.TargetFile,
    args.File,
    args.DirectoryPath,
    args.SearchDirectory,
    args.SearchPath,
    args.file_path,
    args.path,
  ].filter((value) => value !== undefined);
  const extra = Object.entries(args)
    .filter(
      ([key, value]) =>
        typeof value === "string" &&
        /(?:path|file|dir|directory|root)$/iu.test(key) &&
        !explicit.includes(value),
    )
    .map(([, value]) => value);
  const candidates = [...explicit, ...extra];
  if (candidates.length === 0) {
    if (["Glob", "Grep", "grep_search", "find_by_name"].includes(toolName)) {
      return { paths: [fallbackCwd] };
    }
    return { problem: `${toolName}의 경로 인자가 없다` };
  }
  if (candidates.some((value) => typeof value !== "string" || value === "")) {
    return { problem: `${toolName}의 경로 인자가 문자열이 아니다` };
  }
  return { paths: candidates };
};

const commandFromArgs = (args) => {
  if (!plainObject(args)) throw new Error("셸 도구 인자가 객체가 아니다");
  const value = args.CommandLine ?? args.command ?? args.cmd;
  const command = Array.isArray(value)
    ? value
        .map((word) => {
          if (typeof word !== "string") {
            throw new Error("셸 명령 배열 원소가 문자열이 아니다");
          }
          return `'${word.replaceAll("'", "'\\''")}'`;
        })
        .join(" ")
    : value;
  if (typeof command !== "string" || command.trim() === "") {
    throw new Error("셸 명령이 없다");
  }
  return command;
};

const normalizedToolArgs = (args) => {
  if (!plainObject(args)) return args;
  const extra = {};
  if (Object.hasOwn(args, "Cwd")) extra.cwd = args.Cwd;
  if (Object.hasOwn(args, "WorkingDirectory")) {
    extra.workingDirectory = args.WorkingDirectory;
  }
  return Object.keys(extra).length === 0 ? args : { ...args, ...extra };
};

const isDirectory = (path) => {
  try {
    return statSync(path).isDirectory();
  } catch {
    return false;
  }
};

const extractOptionPath = (text) => {
  const results = [];
  if (text.startsWith("@") && text.length > 1) {
    results.push(text.slice(1));
    return results;
  }
  const equals = text.indexOf("=");
  if (text.startsWith("-")) {
    if (equals !== -1) {
      const value = text.slice(equals + 1);
      results.push(
        value.startsWith("@") && value.length > 1 ? value.slice(1) : value,
      );
      return results;
    }
    const atIndex = text.indexOf("@");
    if (atIndex > 1 && atIndex < text.length - 1) {
      results.push(text.slice(atIndex + 1));
    }
    const attachedHome = /^-[a-zA-Z]((?:~|\$HOME|\$\{HOME\}|\/).*)$/u.exec(
      text,
    );
    if (attachedHome !== null) {
      results.push(attachedHome[1]);
    }
    return results;
  }
  results.push(text);
  if (equals > 0 && equals < text.length - 1) {
    const value = text.slice(equals + 1);
    results.push(
      value.startsWith("@") && value.length > 1 ? value.slice(1) : value,
    );
  }
  return results;
};

const looksLikePath = (text) =>
  text.startsWith("/") ||
  text.startsWith("~") ||
  text.startsWith("./") ||
  text.startsWith("../") ||
  text === ".." ||
  text.startsWith("$HOME") ||
  text.startsWith("${HOME}") ||
  text.includes("/");

const parseGitSubcommand = (args) => {
  let index = 0;
  const chdirs = [];
  while (index < args.length) {
    const word = args[index];
    const text = word.text;
    if (!text.startsWith("-")) {
      return { sub: text, rest: args.slice(index + 1), chdirs };
    }
    if (text === "-C") {
      if (args[index + 1] !== undefined) chdirs.push(args[index + 1]);
      index += 2;
      continue;
    }
    if (text.startsWith("-C") && text.length > 2) {
      const value = text.startsWith("-C=") ? text.slice(3) : text.slice(2);
      chdirs.push({ ...word, text: value });
      index += 1;
      continue;
    }
    const equals = text.indexOf("=");
    const flag = equals === -1 ? text : text.slice(0, equals);
    index += GIT_VALUED_GLOBALS.has(flag) && equals === -1 ? 2 : 1;
  }
  return { sub: undefined, rest: [], chdirs };
};

const isGitMessageWord = (invocation, index) => {
  if (invocation.name !== "git") return false;
  const previous = invocation.args[index - 1]?.text;
  if (previous === "-m" || previous === "--message") return true;
  const current = invocation.args[index]?.text ?? "";
  return current.startsWith("--message=") || /^-m.+$/u.test(current);
};

const shellNestedScripts = (invocation, rules) => {
  const scripts = [];
  if (invocation.script !== undefined) scripts.push(invocation.script);
  const name = invocation.name;
  if (name !== undefined && rules.shells.includes(name)) {
    let index = 0;
    let commandFlag = false;
    while (
      index < invocation.args.length &&
      (invocation.args[index].text.startsWith("-") ||
        invocation.args[index].text.startsWith("+"))
    ) {
      const text = invocation.args[index].text;
      if (text === "--") {
        index += 1;
        break;
      }
      if (/^-[a-zA-Z]*c[a-zA-Z]*$/u.test(text)) commandFlag = true;
      index += ["-o", "+o", "-O", "+O", "--rcfile", "--init-file"].includes(
        text,
      )
        ? 2
        : 1;
    }
    if (commandFlag) {
      const scriptWord = invocation.args[index];
      if (scriptWord !== undefined && !scriptWord.dynamic) {
        scripts.push(scriptWord.text);
      }
    }
    for (const stream of invocation.command.inputs) {
      if (!stream.dynamic) scripts.push(stream.text);
    }
  }
  if (
    name === "eval" &&
    invocation.args.length > 0 &&
    invocation.args.every((word) => !word.dynamic)
  ) {
    scripts.push(invocation.args.map((word) => word.text).join(" "));
  }
  if (name === "trap") {
    const action = invocation.args.find(
      (word) =>
        !word.dynamic && !(word.text.startsWith("-") && word.text !== "-"),
    );
    if (action !== undefined && action.text !== "-" && action.text !== "") {
      scripts.push(action.text);
    }
  }
  return scripts;
};

const dynamicPathRisk = (word) => {
  const raw = word.raw ?? word.text ?? "";
  return (
    raw.startsWith("~") ||
    raw.includes("/") ||
    raw.includes("$HOME") ||
    raw.includes("${HOME") ||
    raw.includes("$PWD") ||
    raw.includes("${PWD") ||
    raw.includes("$'") ||
    raw.includes("$(") ||
    raw.includes("`") ||
    /(?:^|\/)\.\.(?:$|\/)/u.test(raw)
  );
};

const opaqueShellExecFindings = (invocation) => {
  const name = invocation.name;
  if (name === undefined) return [];
  const findings = [];
  const beforeDashDash = [];
  for (const word of invocation.args) {
    if (word.text === "--" && !word.dynamic) break;
    beforeDashDash.push(word);
  }
  if (TAR_COMMANDS.has(name)) {
    const first = beforeDashDash[0]?.text ?? "";
    const oldStyleExtract =
      first.length > 0 &&
      first.length <= 10 &&
      !first.startsWith("-") &&
      /^[a-zA-Z]*x[a-zA-Z]*$/u.test(first);
    const flagExtract = beforeDashDash.some(
      (word) =>
        word.text === "--extract" ||
        word.text === "--get" ||
        (/^-[a-zA-Z]*x[a-zA-Z]*$/u.test(word.text) &&
          !word.text.startsWith("--")),
    );
    if (oldStyleExtract || flagExtract) {
      findings.push(
        finding(
          "antigravity-hook-config",
          `아카이브 해제(${name})는 보호 경로와 .agents/hooks.json을 덮어쓸 수 있어 허용하지 않는다`,
        ),
      );
    }
  }
  if (name === "cpio") {
    const extracts = beforeDashDash.some(
      (word) =>
        word.text === "--extract" ||
        word.text === "--pass-through" ||
        (/^-[a-zA-Z]*[ip][a-zA-Z]*$/u.test(word.text) &&
          !word.text.startsWith("--")),
    );
    if (extracts) {
      findings.push(
        finding(
          "antigravity-hook-config",
          "아카이브 해제(cpio)는 보호 경로와 .agents/hooks.json을 덮어쓸 수 있어 허용하지 않는다",
        ),
      );
    }
  }
  if (name === "pax") {
    const extracts = beforeDashDash.some(
      (word) =>
        word.text === "--read" ||
        (/^-[a-zA-Z]*r[a-zA-Z]*$/u.test(word.text) &&
          !word.text.startsWith("--")),
    );
    if (extracts) {
      findings.push(
        finding(
          "antigravity-hook-config",
          "아카이브 해제(pax)는 보호 경로와 .agents/hooks.json을 덮어쓸 수 있어 허용하지 않는다",
        ),
      );
    }
  }
  if (ARCHIVE_EXTRACT_ONLY.has(name)) {
    findings.push(
      finding(
        "antigravity-hook-config",
        `아카이브·복사 해제 도구(${name})는 보호 경로와 .agents/hooks.json을 덮어쓸 수 있어 허용하지 않는다`,
      ),
    );
  }
  if (
    ["7z", "7za", "7zz"].includes(name) &&
    ["x", "e"].includes(beforeDashDash[0]?.text ?? "")
  ) {
    findings.push(
      finding(
        "antigravity-hook-config",
        `아카이브 해제(${name})는 허용하지 않는다`,
      ),
    );
  }
  if (
    (name === "jar" || name === "ar") &&
    beforeDashDash.some((word) => /^-?[a-zA-Z]*x[a-zA-Z]*$/u.test(word.text))
  ) {
    findings.push(
      finding(
        "antigravity-hook-config",
        `아카이브 해제(${name})는 허용하지 않는다`,
      ),
    );
  }
  if (name === "sqlite3") {
    const dangerous = invocation.args.some(
      (word) =>
        word.dynamic ||
        SQLITE_META_PATTERN.test(word.text.trim()) ||
        ["-init", "-cmd"].includes(flagName(word.text)),
    );
    if (dangerous) {
      findings.push(
        finding(
          "antigravity-hook-config",
          "sqlite3의 셸·파일 실행 옵션(.shell, -cmd, -init 등)은 허용하지 않는다",
        ),
      );
    }
  }
  if (name === "git") {
    const { sub, rest } = parseGitSubcommand(invocation.args);
    if (sub === "grep") {
      for (const word of rest) {
        if (word.text === "--" && !word.dynamic) break;
        if (
          word.text === "-O" ||
          (word.text.startsWith("-O") && !word.text.startsWith("--")) ||
          word.text === "--open-files-in-pager" ||
          word.text.startsWith("--open-files-in-pager=") ||
          (/^-[a-zA-Z]*O/u.test(word.text) && !word.text.startsWith("--"))
        ) {
          findings.push(
            finding(
              "antigravity-hook-config",
              "git grep의 -O/--open-files-in-pager 명령 실행 옵션은 허용하지 않는다",
            ),
          );
          break;
        }
      }
    }
  }
  return findings;
};

const hostFromWord = (text) => {
  const urlMatch =
    /^[a-zA-Z][a-zA-Z0-9+.-]*:\/\/([^/?#@\s]+@)?(\[[^\]]+\]|[^/:?\s]+)/u.exec(
      text,
    );
  if (urlMatch !== null) return urlMatch[2].toLowerCase();
  if (/^https?:\/\//iu.test(text)) return text.toLowerCase();
  return undefined;
};

const bareHostFromWord = (text) => {
  const host = text.split("/")[0]?.split(":")[0]?.toLowerCase() ?? "";
  return /^[a-z0-9-]+(?:\.[a-z0-9-]+)+$/u.test(host) ? host : undefined;
};

const isAllowedNetworkHost = (host, policy) => {
  const clean = host.replace(/^\[|\]$/gu, "").toLowerCase();
  if (LOOPBACK_HOSTS.includes(clean) || LOOPBACK_HOSTS.includes(host)) {
    return true;
  }
  if (clean === policy.tossHost || clean.endsWith(`.${policy.tossHost}`)) {
    return false;
  }
  return policy.allowedDomains.some(
    (domain) => clean === domain || clean.endsWith(`.${domain}`),
  );
};

const cargoPackageOrBinTargets = (args) => {
  const targets = [];
  for (let index = 0; index < args.length; index += 1) {
    const word = args[index];
    const text = word.text;
    if (text === "--") break;
    if (["-p", "--package", "--bin", "--manifest-path", "-C"].includes(text)) {
      const next = args[index + 1];
      if (next !== undefined) {
        targets.push({
          flag: text,
          value: next.text,
          dynamic: Boolean(next.dynamic),
        });
      }
      index += 1;
    } else if (
      text.startsWith("--package=") ||
      text.startsWith("--bin=") ||
      text.startsWith("--manifest-path=") ||
      text.startsWith("-p=") ||
      text.startsWith("-C=")
    ) {
      const sep = text.indexOf("=");
      targets.push({
        flag: text.slice(0, sep),
        value: text.slice(sep + 1),
        dynamic: Boolean(word.dynamic),
      });
    } else if (text.startsWith("-p") && text.length > 2) {
      targets.push({
        flag: "-p",
        value: text.slice(2),
        dynamic: Boolean(word.dynamic),
      });
    } else if (text.startsWith("-C") && text.length > 2) {
      targets.push({
        flag: "-C",
        value: text.slice(2),
        dynamic: Boolean(word.dynamic),
      });
    }
  }
  return targets;
};

const cargoPackageMatches = (raw, names) => {
  const lower = raw.toLowerCase();
  const afterHash = lower.includes("#")
    ? lower.slice(lower.indexOf("#") + 1)
    : lower;
  const pkgName = afterHash.split(/[@:]/u)[0];
  if (names.includes(pkgName)) return true;
  return names.some((name) => lower.includes(`/${name}`));
};

const curlShortOptionFindings = (args) => {
  const findings = [];
  const skipIndexes = new Set();
  for (let index = 0; index < args.length; index += 1) {
    const text = args[index].text;
    if (text === "--") break;
    if (text.startsWith("--")) {
      const option = flagName(text);
      if (CURL_DENIED_FLAGS.has(option)) {
        findings.push(
          finding(
            ["--netrc", "--netrc-optional", "--netrc-file"].includes(option)
              ? "protected-home-read"
              : "toss-live-api",
            `curl의 금지 옵션(${option})은 허용하지 않는다`,
          ),
        );
      }
      if (CURL_VALUED_FLAGS.has(text)) {
        skipIndexes.add(index + 1);
        index += 1;
      }
      continue;
    }
    if (!text.startsWith("-") || text === "-") continue;
    const cluster = text.slice(1);
    for (let pos = 0; pos < cluster.length; pos += 1) {
      const letter = cluster[pos];
      if (CURL_SHORT_DENIED_READ.has(letter)) {
        findings.push(
          finding(
            "protected-home-read",
            `curl의 금지 옵션(-${letter})은 허용하지 않는다`,
          ),
        );
        continue;
      }
      if (CURL_SHORT_DENIED_NET.has(letter)) {
        findings.push(
          finding(
            "toss-live-api",
            `curl의 금지 옵션(-${letter})은 허용하지 않는다`,
          ),
        );
        if (pos === cluster.length - 1) {
          skipIndexes.add(index + 1);
          index += 1;
        }
        break;
      }
      if (CURL_SHORT_VALUED.has(letter)) {
        if (pos === cluster.length - 1) {
          skipIndexes.add(index + 1);
          index += 1;
        }
        break;
      }
    }
  }
  return { findings, skipIndexes };
};

const wgetShortOptionFindings = (args) => {
  const findings = [];
  const skipIndexes = new Set();
  for (let index = 0; index < args.length; index += 1) {
    const text = args[index].text;
    if (text === "--") break;
    if (text.startsWith("--")) {
      const option = flagName(text);
      if (WGET_DENIED_FLAGS.has(option)) {
        findings.push(
          finding(
            option === "--netrc-file" ? "protected-home-read" : "toss-live-api",
            `wget의 금지 옵션(${option})은 허용하지 않는다`,
          ),
        );
      }
      continue;
    }
    if (!text.startsWith("-") || text === "-") continue;
    const cluster = text.slice(1);
    for (let pos = 0; pos < cluster.length; pos += 1) {
      const letter = cluster[pos];
      if (WGET_SHORT_DENIED.has(letter)) {
        findings.push(
          finding(
            "toss-live-api",
            `wget의 금지 옵션(-${letter})은 허용하지 않는다`,
          ),
        );
        if (pos === cluster.length - 1) {
          skipIndexes.add(index + 1);
          index += 1;
        }
        break;
      }
      if (WGET_SHORT_VALUED.has(letter)) {
        if (pos === cluster.length - 1) {
          skipIndexes.add(index + 1);
          index += 1;
        }
        break;
      }
    }
  }
  return { findings, skipIndexes };
};

const cargoSubcommand = (args) => {
  for (let index = 0; index < args.length; index += 1) {
    const text = args[index].text;
    if (text === "--") return undefined;
    if (text.startsWith("+")) continue;
    if (text.startsWith("-")) {
      const equals = text.indexOf("=");
      const flag = equals === -1 ? text : text.slice(0, equals);
      if (CARGO_VALUED_GLOBALS.has(flag) && equals === -1) {
        index += 1;
      }
      continue;
    }
    return text;
  }
  return undefined;
};

const envChdirTargets = (words) => {
  const targets = [];
  for (let index = 0; index < words.length; index += 1) {
    const text = words[index].text;
    if (text === "-C" || text === "--chdir") {
      if (words[index + 1] !== undefined) targets.push(words[index + 1].text);
      index += 1;
    } else if (text.startsWith("--chdir=")) {
      targets.push(text.slice("--chdir=".length));
    }
  }
  return targets;
};

const flagName = (text) => {
  const equals = text.indexOf("=");
  return equals === -1 ? text : text.slice(0, equals);
};

const isRemoteRsyncOperand = (text) =>
  !text.startsWith("-") &&
  (text.startsWith("rsync://") || /^[^/\s]+:.*$/u.test(text));

const analyzeScriptTossAndReads = (source, context, depth = 0) => {
  if (depth > 8) {
    return [finding("unparsable", "명령 중첩 깊이가 상한을 넘었다")];
  }
  const parsed = parseScript(source);
  const findings = [];
  let cwd = context.cwd;
  const checkReadCandidate = (raw, baseCwd) => {
    findings.push(
      ...readTargetFindings(
        raw,
        baseCwd,
        context.policy,
        context.ownedWorktree,
        context.home,
      ),
    );
  };
  for (const command of parsed.commands) {
    const invocation = unwrap(command, DEFINITIONS, context.rules);
    if (invocation.wrappers.includes("env")) {
      for (const chdir of envChdirTargets(command.words)) {
        checkReadCandidate(chdir, cwd);
      }
    }
    for (const assignment of invocation.env) {
      if (/^TOSS_/iu.test(assignment.name)) {
        findings.push(
          finding(
            "toss-live-api",
            `실제 토스 자격 증명·설정 환경 변수(${assignment.name})는 에이전트 셸에서 설정하지 않는다`,
          ),
        );
      }
      const valueText = assignment.value?.text ?? "";
      if (assignment.name === "PATH") {
        for (const piece of valueText.split(":")) {
          if (piece && looksLikePath(piece)) checkReadCandidate(piece, cwd);
        }
      } else {
        if (
          assignment.name !== "TMPDIR" &&
          assignment.value?.dynamic &&
          dynamicPathRisk(assignment.value)
        ) {
          findings.push(
            finding(
              "protected-home-read",
              `동적 경로 대입(${assignment.name}=${assignment.value.raw})은 허용하지 않는다`,
            ),
          );
        }
        if (valueText && looksLikePath(valueText)) {
          checkReadCandidate(valueText, cwd);
        }
      }
      if (valueText.toLowerCase().includes(context.policy.tossHost)) {
        findings.push(
          finding(
            "toss-live-api",
            `토스 운영 호스트(${context.policy.tossHost})를 환경 변수로 전달하지 않는다`,
          ),
        );
      }
    }
    if (
      invocation.name !== undefined &&
      context.rules.environment.exporters.includes(invocation.name)
    ) {
      for (const word of invocation.args) {
        const equals = word.text.indexOf("=");
        const name = (
          equals === -1 ? word.text : word.text.slice(0, equals)
        ).replace(/\+$/u, "");
        const valueText = equals === -1 ? "" : word.text.slice(equals + 1);
        if (/^TOSS_/iu.test(name)) {
          findings.push(
            finding(
              "toss-live-api",
              `실제 토스 환경 변수(${name})를 내보내지 않는다`,
            ),
          );
        }
        if (name === "PATH") {
          for (const piece of valueText.split(":")) {
            if (piece && looksLikePath(piece)) checkReadCandidate(piece, cwd);
          }
        } else {
          if (name !== "TMPDIR" && word.dynamic && dynamicPathRisk(word)) {
            findings.push(
              finding(
                "protected-home-read",
                `동적 경로 내보내기(${word.raw})는 허용하지 않는다`,
              ),
            );
          }
          if (valueText && looksLikePath(valueText)) {
            checkReadCandidate(valueText, cwd);
          }
        }
        if (valueText.toLowerCase().includes(context.policy.tossHost)) {
          findings.push(
            finding(
              "toss-live-api",
              `토스 운영 호스트(${context.policy.tossHost})를 환경 변수로 내보내지 않는다`,
            ),
          );
        }
      }
    }
    for (const redir of command.redirects) {
      if (["<", "<>", "<&"].includes(redir.operator)) {
        if (redir.target.dynamic) {
          findings.push(
            finding(
              "protected-home-read",
              `동적 입력 리다이렉션 대상(${redir.target.raw})은 허용하지 않는다`,
            ),
          );
        } else {
          checkReadCandidate(
            redir.target.text,
            invocation.cwdUnknown ? undefined : cwd,
          );
        }
      }
    }
    findings.push(...opaqueShellExecFindings(invocation));
    const name = invocation.name;
    const effectiveCwd = invocation.cwdUnknown ? undefined : cwd;
    if (name === "security") {
      findings.push(
        finding(
          "protected-home-read",
          "macOS 키체인 도구(security)는 에이전트 셸에서 허용하지 않는다",
        ),
      );
    }
    if (name !== undefined && TOSS_LIVE_BINARIES.includes(name)) {
      findings.push(
        finding(
          "toss-live-api",
          `실제 API·앱 실행 바이너리(${name})는 에이전트가 직접 실행하지 않는다`,
        ),
      );
    }
    if (name === "open") {
      findings.push(
        finding(
          "toss-live-api",
          "macOS 앱·URL 실행 명령(open)은 에이전트 셸에서 허용하지 않는다",
        ),
      );
    }
    if (name === "cargo") {
      const sub = cargoSubcommand(invocation.args);
      if (sub === "run" || sub === "r") {
        const targets = cargoPackageOrBinTargets(invocation.args);
        const hasDynamicCargoTarget = targets.some((item) => item.dynamic);
        if (hasDynamicCargoTarget) {
          findings.push(
            finding(
              "toss-live-api",
              "cargo run의 동적 패키지·바이너리·매니페스트 지정은 허용하지 않는다",
            ),
          );
        }
        const packages = targets
          .filter((item) => ["-p", "--package"].includes(item.flag))
          .map((item) => item.value);
        const bins = targets
          .filter((item) => item.flag === "--bin")
          .map((item) => item.value);
        const manifests = targets
          .filter((item) => item.flag === "--manifest-path")
          .map((item) => item.value);
        const cargoDirs = targets
          .filter((item) => item.flag === "-C")
          .map((item) => item.value);
        const resolvedCargoCwd =
          cargoDirs.length > 0 && effectiveCwd !== undefined
            ? resolve(effectiveCwd, ...cargoDirs)
            : effectiveCwd;
        const cwdLower = (resolvedCargoCwd ?? "").toLowerCase();
        const cwdIsToss =
          packages.length === 0 &&
          manifests.length === 0 &&
          (cwdLower.includes("/toss-smoke") ||
            cwdLower.includes("/broker-toss-trade"));
        if (
          cwdIsToss ||
          packages.some((pkg) =>
            cargoPackageMatches(pkg, ["toss-smoke", "broker-toss-trade"]),
          ) ||
          bins.some((bin) =>
            cargoPackageMatches(bin, ["toss-smoke", "broker-toss-trade"]),
          ) ||
          manifests.some(
            (manifest) =>
              manifest.toLowerCase().includes("toss-smoke") ||
              manifest.toLowerCase().includes("broker-toss-trade"),
          )
        ) {
          findings.push(
            finding(
              "toss-live-api",
              "실제 토스 API·주문 crate(toss-smoke, broker-toss-trade)는 에이전트가 실행하지 않는다",
            ),
          );
        }
        const cwdIsTauri =
          packages.length === 0 &&
          manifests.length === 0 &&
          cwdLower.includes("/src-tauri");
        const runsTauri =
          cwdIsTauri ||
          packages.some((pkg) => cargoPackageMatches(pkg, ["src-tauri"])) ||
          bins.some((bin) => cargoPackageMatches(bin, ["src-tauri"])) ||
          manifests.some((manifest) =>
            manifest.toLowerCase().includes("src-tauri"),
          );
        const onlyGenBindings = bins.length === 1 && bins[0] === "gen-bindings";
        if (runsTauri && !onlyGenBindings) {
          findings.push(
            finding(
              "toss-live-api",
              "실제 앱 바이너리(src-tauri)는 단독 --bin gen-bindings 외에 에이전트가 직접 실행하지 않는다",
            ),
          );
        }
      }
      if (sub === "tauri") {
        const nonOptions = invocation.args.filter(
          (word) => !word.text.startsWith("-") && !word.text.startsWith("+"),
        );
        if (nonOptions.at(1)?.text === "dev") {
          findings.push(
            finding(
              "toss-live-api",
              "tauri dev 실앱 실행은 에이전트가 하지 않는다",
            ),
          );
        }
      }
    }
    if (
      name === "tauri" &&
      invocation.args.some((word) => word.text === "dev")
    ) {
      findings.push(
        finding(
          "toss-live-api",
          "tauri dev 실앱 실행은 에이전트가 하지 않는다",
        ),
      );
    }
    if (
      ["pnpm", "npm", "yarn", "npx"].includes(name ?? "") &&
      invocation.args.some((word) => word.text === "tauri") &&
      invocation.args.some((word) => word.text === "dev")
    ) {
      findings.push(
        finding(
          "toss-live-api",
          "tauri dev 실앱 실행은 에이전트가 하지 않는다",
        ),
      );
    }
    if (
      name === "rsync" &&
      invocation.args.some((word) => isRemoteRsyncOperand(word.text))
    ) {
      findings.push(
        finding("toss-live-api", "원격 rsync 연결은 허용하지 않는다"),
      );
    }
    if (name !== undefined && NETWORK_CLIENTS.includes(name)) {
      let skipOptionValues = new Set();
      if (name === "curl") {
        const scanned = curlShortOptionFindings(invocation.args);
        findings.push(...scanned.findings);
        skipOptionValues = scanned.skipIndexes;
      }
      if (name === "wget") {
        const scanned = wgetShortOptionFindings(invocation.args);
        findings.push(...scanned.findings);
        skipOptionValues = scanned.skipIndexes;
      }
      const clientText = invocation.args.map((word) => word.text);
      const hasToss = clientText.some((text) =>
        text.toLowerCase().includes(context.policy.tossHost),
      );
      if (hasToss) {
        findings.push(
          finding(
            "toss-live-api",
            `네트워크 도구(${name})로 토스 운영 호스트(${context.policy.tossHost})에 연결하지 않는다`,
          ),
        );
      } else {
        for (let index = 0; index < invocation.args.length; index += 1) {
          const word = invocation.args[index];
          if (word.dynamic) {
            findings.push(
              finding(
                "toss-live-api",
                `네트워크 도구(${name})의 동적 대상(${word.raw})은 허용하지 않는다`,
              ),
            );
            break;
          }
          if (name === "curl" && word.text === "--url") {
            const targetUrl = invocation.args[index + 1];
            index += 1;
            const host =
              targetUrl === undefined
                ? undefined
                : (hostFromWord(targetUrl.text) ??
                  bareHostFromWord(targetUrl.text));
            if (
              targetUrl === undefined ||
              targetUrl.dynamic ||
              host === undefined ||
              !isAllowedNetworkHost(host, context.policy)
            ) {
              findings.push(
                finding(
                  "toss-live-api",
                  `네트워크 도구(curl)로 허용 목록 밖 대상(${targetUrl?.text ?? ""})에 연결하지 않는다`,
                ),
              );
            }
            continue;
          }
          if (name === "curl" && word.text.startsWith("--url=")) {
            const rawUrl = word.text.slice("--url=".length);
            const host = hostFromWord(rawUrl) ?? bareHostFromWord(rawUrl);
            if (
              host === undefined ||
              !isAllowedNetworkHost(host, context.policy)
            ) {
              findings.push(
                finding(
                  "toss-live-api",
                  `네트워크 도구(curl)로 허용 목록 밖 대상(${rawUrl})에 연결하지 않는다`,
                ),
              );
            }
            continue;
          }
          if (skipOptionValues.has(index)) continue;
          if (
            HTTP_CLIENTS.has(name) &&
            name !== "curl" &&
            name !== "wget" &&
            CURL_VALUED_FLAGS.has(word.text)
          ) {
            index += 1;
            continue;
          }
          if (word.text.startsWith("-")) continue;
          const host =
            hostFromWord(word.text) ??
            (HTTP_CLIENTS.has(name) ? bareHostFromWord(word.text) : undefined);
          if (HTTP_CLIENTS.has(name)) {
            if (
              host === undefined ||
              !isAllowedNetworkHost(host, context.policy)
            ) {
              findings.push(
                finding(
                  "toss-live-api",
                  `네트워크 도구(${name})로 허용 목록 밖 호스트(${host ?? word.text})에 연결하지 않는다`,
                ),
              );
            }
          } else if (
            host !== undefined &&
            !isAllowedNetworkHost(host, context.policy)
          ) {
            findings.push(
              finding(
                "toss-live-api",
                `네트워크 도구(${name})로 허용 목록 밖 호스트(${host})에 연결하지 않는다`,
              ),
            );
          }
        }
        if (RAW_REMOTE_CLIENTS.has(name)) {
          const positionals = invocation.args.filter(
            (word) => !word.text.startsWith("-"),
          );
          for (const positional of positionals) {
            const candidate =
              positional.text.split("@").at(-1)?.split(":")[0] ?? "";
            if (
              candidate !== "" &&
              !isAllowedNetworkHost(candidate, context.policy)
            ) {
              findings.push(
                finding(
                  "toss-live-api",
                  `네트워크 도구(${name})로 허용 목록 밖 원격 대상(${positional.text})에 연결하지 않는다`,
                ),
              );
              break;
            }
          }
        }
        if (
          name === "openssl" &&
          invocation.args.some(
            (word) => word.text === "s_client" || word.text === "s_time",
          )
        ) {
          findings.push(
            finding(
              "toss-live-api",
              "openssl 외부 연결 하위 명령(s_client, s_time)은 허용하지 않는다",
            ),
          );
        }
      }
    }
    const isPureEcho = ["echo", "printf"].includes(name ?? "");
    const gitSub =
      name === "git" ? parseGitSubcommand(invocation.args).sub : undefined;
    const isGitNetwork =
      gitSub !== undefined && GIT_NETWORK_SUBCOMMANDS.has(gitSub);
    const isExporter =
      name !== undefined && context.rules.environment.exporters.includes(name);
    const isReader =
      name !== undefined &&
      name !== "open" &&
      !isGitNetwork &&
      (context.rules.readers.includes(name) ||
        name === "git" ||
        name === "just");
    for (let index = 0; index < invocation.args.length; index += 1) {
      const word = invocation.args[index];
      const text = word.text;
      if (!isReader && text.toLowerCase().includes(context.policy.tossHost)) {
        findings.push(
          finding(
            "toss-live-api",
            `실행 명령(${name ?? ""})에 토스 운영 호스트(${context.policy.tossHost})를 넘기지 않는다`,
          ),
        );
      }
      if (isPureEcho || isGitMessageWord(invocation, index)) continue;
      if (isExporter) {
        const equals = text.indexOf("=");
        const exportedName = (
          equals === -1 ? text : text.slice(0, equals)
        ).replace(/\+$/u, "");
        if (exportedName === "PATH" || exportedName === "TMPDIR") continue;
      }
      if (word.dynamic) {
        if (
          (name !== undefined && FILE_READ_COMMANDS.has(name)) ||
          dynamicPathRisk(word)
        ) {
          findings.push(
            finding(
              "protected-home-read",
              `동적 읽기·경로 인자(${word.raw})는 홈·자격 증명 우회 방지를 위해 허용하지 않는다`,
            ),
          );
          continue;
        }
      }
      for (const candidate of extractOptionPath(text)) {
        if (candidate && looksLikePath(candidate)) {
          checkReadCandidate(candidate, effectiveCwd);
        } else if (
          candidate &&
          effectiveCwd !== undefined &&
          !candidate.startsWith("-")
        ) {
          const joined = resolve(effectiveCwd, candidate);
          findings.push(
            ...readTargetFindings(
              joined,
              effectiveCwd,
              context.policy,
              context.ownedWorktree,
              context.home,
            ),
          );
        }
      }
    }
    if (name === "cd" || name === "pushd") {
      const target = invocation.args.find((word) => !word.text.startsWith("-"));
      if (target === undefined) {
        findings.push(
          finding(
            "protected-home-read",
            "인자 없는 cd로 사용자 홈 폴더로 이동하지 않는다",
          ),
        );
        cwd = undefined;
      } else if (target.dynamic || target.glob || target.text === "-") {
        cwd = undefined;
      } else {
        checkReadCandidate(target.text, effectiveCwd);
        const expanded = expandHome(target.text, context.home);
        if (expanded.path !== undefined) {
          const nextDir = isAbsolute(expanded.path)
            ? resolve(expanded.path)
            : effectiveCwd === undefined
              ? undefined
              : resolve(effectiveCwd, expanded.path);
          cwd =
            nextDir !== undefined && isDirectory(nextDir) ? nextDir : undefined;
        } else {
          cwd = undefined;
        }
      }
    } else if (name === "popd") {
      cwd = undefined;
    }
    for (const nested of shellNestedScripts(invocation, context.rules)) {
      findings.push(
        ...analyzeScriptTossAndReads(
          nested,
          { ...context, cwd: effectiveCwd },
          depth + 1,
        ),
      );
    }
  }
  for (const substitution of parsed.substitutions) {
    findings.push(
      ...analyzeScriptTossAndReads(substitution, context, depth + 1),
    );
  }
  return findings;
};

const extraAttachedWriteTargets = (invocation) => {
  if (invocation.name === "curl") {
    return invocation.args.flatMap((word) => {
      if (word.text.startsWith("--output=") && word.text.length > 9) {
        return [{ ...word, text: word.text.slice(9) }];
      }
      if (
        word.text.startsWith("-o") &&
        word.text.length > 2 &&
        !word.text.startsWith("--")
      ) {
        return [{ ...word, text: word.text.slice(2) }];
      }
      return [];
    });
  }
  if (invocation.name === "wget") {
    return invocation.args.flatMap((word) => {
      if (
        word.text.startsWith("-O") &&
        word.text.length > 2 &&
        !word.text.startsWith("--")
      ) {
        return [{ ...word, text: word.text.slice(2) }];
      }
      return [];
    });
  }
  return [];
};

const shellWriteTargets = (
  source,
  cwd,
  project,
  role,
  policy,
  home,
  depth = 0,
) => {
  if (depth > 8) return [];
  const parsed = parseScript(source);
  const findings = [];
  let currentCwd = cwd;
  for (const command of parsed.commands) {
    const invocation = unwrap(command, DEFINITIONS, project.rules);
    const effectiveCwd = invocation.cwdUnknown ? undefined : currentCwd;
    const targets = [
      ...command.redirects
        .filter((item) => !["<", "<&"].includes(item.operator))
        .map((item) => item.target),
      ...(invocation.name !== undefined &&
      Object.hasOwn(WRITERS, invocation.name)
        ? WRITERS[invocation.name](invocation.args)
        : []),
      ...extraAttachedWriteTargets(invocation),
    ];
    for (const target of targets) {
      if (target.dynamic || target.glob) continue;
      findings.push(...homeWriteFindings(target.text, policy, home));
      const expanded = expandHome(target.text, home);
      if (expanded.path === undefined) continue;
      const absolute = isAbsolute(expanded.path)
        ? resolve(expanded.path)
        : effectiveCwd === undefined
          ? undefined
          : resolve(effectiveCwd, expanded.path);
      if (absolute === undefined) continue;
      for (const candidate of [...new Set([absolute, realTarget(absolute)])]) {
        findings.push(...homeWriteFindings(candidate, policy, home));
        findings.push(
          ...outsideWorktreeShellWriteFindings(
            candidate,
            project.ownedWorktree,
            home,
          ),
        );
        const located = locate(candidate, project.worktrees);
        if (located.root !== undefined && located.relative !== undefined) {
          findings.push(...antigravityPathFindings(located.relative));
        }
      }
    }
    if (invocation.name === "cd" || invocation.name === "pushd") {
      const target = invocation.args.find((word) => !word.text.startsWith("-"));
      if (
        target !== undefined &&
        !target.dynamic &&
        !target.glob &&
        target.text !== "-" &&
        effectiveCwd !== undefined
      ) {
        const nextDir = resolve(effectiveCwd, target.text);
        currentCwd = isDirectory(nextDir) ? nextDir : undefined;
      } else {
        currentCwd = undefined;
      }
    } else if (invocation.name === "popd") {
      currentCwd = undefined;
    }
    for (const nested of shellNestedScripts(invocation, project.rules)) {
      findings.push(
        ...shellWriteTargets(
          nested,
          effectiveCwd,
          project,
          role,
          policy,
          home,
          depth + 1,
        ),
      );
    }
  }
  for (const substitution of parsed.substitutions) {
    findings.push(
      ...shellWriteTargets(
        substitution,
        currentCwd,
        project,
        role,
        policy,
        home,
        depth + 1,
      ),
    );
  }
  return findings;
};

const gitPathOperandFindings = (words, gitCwd, project, actor) => {
  const findings = [];
  if (words.length === 0) {
    return [
      finding(
        "antigravity-hook-config",
        "경로 지정 없는 git 파일 제거·복원은 허용하지 않는다",
      ),
    ];
  }
  for (const word of words) {
    if (
      word.dynamic ||
      word.glob ||
      word.text.startsWith(":") ||
      /[*?[\]]/u.test(word.text)
    ) {
      findings.push(
        finding(
          "antigravity-hook-config",
          `동적·와일드카드·pathspec 매직 git 경로 인자(${word.raw})는 허용하지 않는다`,
        ),
      );
      continue;
    }
    if (gitCwd === undefined) {
      findings.push(
        finding(
          "antigravity-hook-config",
          `현재 폴더를 알 수 없어 git 경로 인자(${word.text})를 검증할 수 없다`,
        ),
      );
      continue;
    }
    const absolute = isAbsolute(word.text)
      ? resolve(word.text)
      : resolve(gitCwd, word.text);
    for (const candidate of [...new Set([absolute, realTarget(absolute)])]) {
      if (project.worktrees.some((wt) => candidate === wt)) {
        findings.push(
          finding(
            "antigravity-hook-config",
            `작업 사본 루트(${word.text})를 대상으로 하는 git 변경은 .agents/hooks.json을 훼손할 수 있어 거부한다`,
          ),
        );
        continue;
      }
      const located = locate(candidate, project.worktrees);
      if (located.root !== undefined && located.relative !== undefined) {
        if (located.relative === "" || located.relative === ".") {
          findings.push(
            finding(
              "antigravity-hook-config",
              `작업 사본 루트(${word.text})를 대상으로 하는 git 변경은 허용하지 않는다`,
            ),
          );
        } else {
          findings.push(...antigravityPathFindings(located.relative));
        }
      }
      findings.push(
        ...editFindings(
          { ...actor, tool_input: { file_path: candidate } },
          project,
        ),
      );
    }
  }
  return findings;
};

const gitOperands = (rest, valuedFlags = new Set()) => {
  const operands = [];
  let afterDashDash = false;
  for (let index = 0; index < rest.length; index += 1) {
    const word = rest[index];
    if (!afterDashDash && word.text === "--" && !word.dynamic) {
      afterDashDash = true;
      continue;
    }
    if (!afterDashDash && word.text.startsWith("-") && word.text !== "-") {
      const equals = word.text.indexOf("=");
      const flag = equals === -1 ? word.text : word.text.slice(0, equals);
      if (valuedFlags.has(flag) && equals === -1) index += 1;
      continue;
    }
    operands.push(word);
  }
  return operands;
};

const gitProtectedMutationFindings = (
  source,
  cwd,
  project,
  actor,
  role,
  depth = 0,
) => {
  if (depth > 8) return [];
  const parsed = parseScript(source);
  const findings = [];
  let currentCwd = cwd;
  for (const command of parsed.commands) {
    const invocation = unwrap(command, DEFINITIONS, project.rules);
    const effectiveCwd = invocation.cwdUnknown ? undefined : currentCwd;
    if (invocation.name === "git") {
      const { sub, rest, chdirs } = parseGitSubcommand(invocation.args);
      let gitCwd = effectiveCwd;
      for (const dirWord of chdirs) {
        if (dirWord.dynamic || dirWord.glob || gitCwd === undefined) {
          gitCwd = undefined;
          break;
        }
        gitCwd = resolve(gitCwd, dirWord.text);
      }
      const hasPathspecFile = rest.some((word) =>
        word.text.startsWith("--pathspec-from-file"),
      );
      if (sub === "rm" || sub === "mv") {
        if (hasPathspecFile) {
          findings.push(
            finding(
              "antigravity-hook-config",
              `git ${sub}의 --pathspec-from-file은 허용하지 않는다`,
            ),
          );
        } else {
          const operands = gitOperands(rest);
          findings.push(
            ...gitPathOperandFindings(operands, gitCwd, project, actor),
          );
        }
      }
      if (sub === "switch" && role !== ROLE.reviewer && role !== ROLE.unknown) {
        findings.push(
          finding(
            "antigravity-hook-config",
            "Antigravity 구현자는 git switch로 브랜치나 커밋을 전환하지 않는다",
          ),
        );
      }
      if (
        sub === "checkout" &&
        role !== ROLE.reviewer &&
        role !== ROLE.unknown
      ) {
        const dashDash = rest.findIndex(
          (word) => word.text === "--" && !word.dynamic,
        );
        if (dashDash === -1) {
          findings.push(
            finding(
              "antigravity-hook-config",
              "Antigravity 구현자는 git checkout으로 커밋·브랜치를 전환하지 않는다. 단일 파일 복원은 -- <경로> 형식만 사용한다",
            ),
          );
        } else {
          findings.push(
            ...gitPathOperandFindings(
              rest.slice(dashDash + 1),
              gitCwd,
              project,
              actor,
            ),
          );
        }
      }
      if (sub === "restore") {
        if (hasPathspecFile) {
          findings.push(
            finding(
              "antigravity-hook-config",
              "git restore의 --pathspec-from-file은 허용하지 않는다",
            ),
          );
        } else {
          const operands = gitOperands(rest, new Set(["-s", "--source"]));
          findings.push(
            ...gitPathOperandFindings(operands, gitCwd, project, actor),
          );
        }
      }
      if (sub === "stash") {
        const stashAction = rest.find((word) => !word.text.startsWith("-"));
        const isReadOnlyStash =
          stashAction !== undefined &&
          !stashAction.dynamic &&
          ["list", "show"].includes(stashAction.text);
        if (!isReadOnlyStash) {
          findings.push(
            finding(
              "antigravity-hook-config",
              "변경형 git stash는 작업 사본과 .agents/hooks.json을 되돌릴 수 있어 허용하지 않는다",
            ),
          );
        }
      }
      if (sub === "clean") {
        const dryRun = rest.some(
          (word) => word.text === "-n" || word.text === "--dry-run",
        );
        const forced = rest.some(
          (word) =>
            word.text === "--force" ||
            (/^-[a-zA-Z]*f[a-zA-Z]*$/u.test(word.text) &&
              !word.text.startsWith("--")),
        );
        if (!dryRun || forced) {
          findings.push(
            finding(
              "antigravity-hook-config",
              "실제 파일을 삭제하는 git clean은 허용하지 않는다",
            ),
          );
        }
      }
      if (sub === "checkout-index" || sub === "read-tree") {
        findings.push(
          finding(
            "antigravity-hook-config",
            `작업 트리·인덱스를 직접 덮어쓰는 git ${sub}는 허용하지 않는다`,
          ),
        );
      }
      if (sub === "reset") {
        if (
          rest.some((word) =>
            ["--merge", "--keep"].includes(flagName(word.text)),
          )
        ) {
          findings.push(
            finding(
              "antigravity-hook-config",
              "작업 트리를 되돌리는 git reset --merge/--keep은 허용하지 않는다",
            ),
          );
        }
        const dashDash = rest.findIndex(
          (word) => word.text === "--" && !word.dynamic,
        );
        if (dashDash !== -1) {
          findings.push(
            ...gitPathOperandFindings(
              rest.slice(dashDash + 1),
              gitCwd,
              project,
              actor,
            ),
          );
        }
      }
    }
    if (invocation.name === "cd" || invocation.name === "pushd") {
      const target = invocation.args.find((word) => !word.text.startsWith("-"));
      if (
        target !== undefined &&
        !target.dynamic &&
        !target.glob &&
        target.text !== "-" &&
        effectiveCwd !== undefined
      ) {
        const nextDir = resolve(effectiveCwd, target.text);
        currentCwd = isDirectory(nextDir) ? nextDir : undefined;
      } else {
        currentCwd = undefined;
      }
    } else if (invocation.name === "popd") {
      currentCwd = undefined;
    }
    for (const nested of shellNestedScripts(invocation, project.rules)) {
      findings.push(
        ...gitProtectedMutationFindings(
          nested,
          effectiveCwd,
          project,
          actor,
          role,
          depth + 1,
        ),
      );
    }
  }
  for (const substitution of parsed.substitutions) {
    findings.push(
      ...gitProtectedMutationFindings(
        substitution,
        currentCwd,
        project,
        actor,
        role,
        depth + 1,
      ),
    );
  }
  return findings;
};

const reviewerGitFindings = (source, role, rules, depth = 0) => {
  if ((role !== ROLE.reviewer && role !== ROLE.unknown) || depth > 8) return [];
  const parsed = parseScript(source);
  const findings = [];
  for (const command of parsed.commands) {
    const invocation = unwrap(command, DEFINITIONS, rules);
    if (invocation.name === "git") {
      const { sub, rest } = parseGitSubcommand(invocation.args);
      if (sub !== undefined && REVIEWER_DENIED_GIT_SUBCOMMANDS.has(sub)) {
        findings.push(
          finding(
            "role-without-writes",
            `리뷰어·비판자는 git ${sub}로 작업 사본이나 이력을 바꾸지 않는다`,
          ),
        );
      }
      if (sub === "branch") {
        const readOnly =
          rest.length === 0 ||
          rest.every((word) => {
            const flag = word.text.split("=")[0];
            return REVIEWER_READONLY_BRANCH_FLAGS.has(flag);
          });
        if (!readOnly) {
          findings.push(
            finding(
              "role-without-writes",
              "리뷰어·비판자는 git branch로 브랜치를 만들거나 수정·삭제하지 않는다",
            ),
          );
        }
      }
      if (sub === "checkout" || sub === "switch") {
        const deniedCheckoutFlag = rest.some(
          (word) =>
            word.dynamic ||
            word.text === "--" ||
            [
              "-b",
              "-B",
              "--orphan",
              "-f",
              "--force",
              "--ours",
              "--theirs",
              "-p",
              "--patch",
            ].includes(flagName(word.text)),
        );
        const positionals = rest.filter((word) => !word.text.startsWith("-"));
        const isDetach =
          sub === "checkout" &&
          !deniedCheckoutFlag &&
          positionals.length === 1 &&
          rest.some((word) => word.text === "--detach" || word.text === "-d");
        if (!isDetach) {
          findings.push(
            finding(
              "role-without-writes",
              `리뷰어·비판자는 git ${sub} 대신 git checkout --detach <SHA>만 사용한다`,
            ),
          );
        }
      }
    }
    for (const nested of shellNestedScripts(invocation, rules)) {
      findings.push(...reviewerGitFindings(nested, role, rules, depth + 1));
    }
  }
  for (const substitution of parsed.substitutions) {
    findings.push(...reviewerGitFindings(substitution, role, rules, depth + 1));
  }
  return findings;
};

const isStaticPrelockGenerator = (source) => {
  const parsed = parseScript(source);
  if (
    parsed.problems.length > 0 ||
    parsed.substitutions.length > 0 ||
    parsed.commands.length !== 1
  ) {
    return false;
  }
  const command = parsed.commands[0];
  if (
    command.piped ||
    command.redirects.length > 0 ||
    command.inputs.length > 0 ||
    command.assignments.length > 0
  ) {
    return false;
  }
  if (
    command.words.length !== 3 ||
    command.words.some((word) => word.dynamic)
  ) {
    return false;
  }
  const [bin, script, flag] = command.words.map((word) => word.text);
  const normalizedScript = script.replace(/^(?:\.\/)+/u, "");
  return (
    basename(bin).toLowerCase() === "node" &&
    PRELOCK_GENERATORS.has(normalizedScript) &&
    flag === "--write"
  );
};

const lockedScriptMatches = (text, target, cwd, worktrees) => {
  const normalized = text.replace(/^(?:\.\/)+/u, "");
  if (normalized === target || normalized.endsWith(`/${target}`)) {
    return true;
  }
  const absolute = isAbsolute(text)
    ? resolve(text)
    : cwd === undefined
      ? undefined
      : resolve(cwd, text);
  if (absolute === undefined) {
    return basename(text) === basename(target);
  }
  const real = realOrSelf(absolute);
  return worktrees.some(
    (root) =>
      realOrSelf(join(root, target)) === real ||
      join(root, target) === absolute,
  );
};

const nodeScriptArg = (args, valuedOptions) => {
  let index = 0;
  while (index < args.length) {
    const word = args[index];
    if (word.dynamic) return { index, word };
    if (word.text === "--") {
      return index + 1 < args.length
        ? { index: index + 1, word: args[index + 1] }
        : undefined;
    }
    if (!word.text.startsWith("-") || word.text === "-") {
      return { index, word };
    }
    index += valuedOptions.includes(word.text) ? 2 : 1;
  }
  return undefined;
};

const lockedConfigGeneratorFindings = (
  source,
  lock,
  rules,
  cwd,
  worktrees,
  depth = 0,
) => {
  if (lock === LOCK.off || depth > 8) return [];
  const parsed = parseScript(source);
  const findings = [];
  let currentCwd = cwd;
  for (const command of parsed.commands) {
    const invocation = unwrap(command, DEFINITIONS, rules);
    const effectiveCwd = invocation.cwdUnknown ? undefined : currentCwd;
    if (invocation.name === "node") {
      const found = nodeScriptArg(
        invocation.args,
        rules.harnessNodeValueOptions ?? [],
      );
      if (found !== undefined) {
        const rest = invocation.args.slice(found.index + 1);
        const writes = rest.some(
          (word) => word.dynamic || word.text === "--write",
        );
        const matchesGenerator =
          found.word.dynamic ||
          [...LOCKED_CONFIG_GENERATORS].some((target) =>
            lockedScriptMatches(
              found.word.text,
              target,
              effectiveCwd,
              worktrees,
            ),
          );
        if (writes && matchesGenerator) {
          findings.push(
            finding(
              "harness-write-mode",
              `${found.word.text} --write는 잠금 on에서 허용하지 않는다`,
            ),
          );
        }
      }
    }
    if (invocation.name === "cd" || invocation.name === "pushd") {
      const target = invocation.args.find((word) => !word.text.startsWith("-"));
      if (
        target !== undefined &&
        !target.dynamic &&
        !target.glob &&
        target.text !== "-" &&
        effectiveCwd !== undefined
      ) {
        const nextDir = resolve(effectiveCwd, target.text);
        currentCwd = isDirectory(nextDir) ? nextDir : undefined;
      } else {
        currentCwd = undefined;
      }
    } else if (invocation.name === "popd") {
      currentCwd = undefined;
    }
    for (const nested of shellNestedScripts(invocation, rules)) {
      findings.push(
        ...lockedConfigGeneratorFindings(
          nested,
          lock,
          rules,
          effectiveCwd,
          worktrees,
          depth + 1,
        ),
      );
    }
  }
  for (const substitution of parsed.substitutions) {
    findings.push(
      ...lockedConfigGeneratorFindings(
        substitution,
        lock,
        rules,
        currentCwd,
        worktrees,
        depth + 1,
      ),
    );
  }
  return findings;
};

const patchFindings = (patch, cwd, actor, project, owned, policy, home) =>
  patchPaths(patch).flatMap((path) => {
    const homeDenied = homeWriteFindings(path, policy, home);
    if (homeDenied.length > 0) return homeDenied;
    const absolute = resolve(cwd, path);
    const resolvedHomeDenied = homeWriteFindings(absolute, policy, home);
    if (resolvedHomeDenied.length > 0) return resolvedHomeDenied;
    if (!absolute.startsWith(`${owned}/`)) {
      throw new Error("세션 작업 사본 밖의 편집이다");
    }
    const located = locate(absolute, project.worktrees);
    const extra =
      located.relative === undefined
        ? []
        : antigravityPathFindings(located.relative);
    return [
      ...extra,
      ...editFindings(
        { ...actor, tool_input: { file_path: absolute } },
        project,
      ),
    ];
  });

const DELEGATE_TASK_TOOLS = Object.freeze([
  "mcp_t3-code_delegate_task",
  "mcp__t3_code__delegate_task",
  "mcp__t3-code__delegate_task",
  "delegate_task",
]);

const DELEGATE_ALLOWED_KEYS = Object.freeze([
  "task",
  "title",
  "target",
  "isolation",
  "mode",
  "role",
  "runtimeMode",
  "interactionMode",
  "clientRequestId",
  "timeoutMs",
  "toolSummary",
]);

const readAgentRules = (projectRoot) => {
  try {
    const parsed = JSON.parse(
      readFileSync(
        join(projectRoot, "harness/claude-hooks/agent-rules.json"),
        "utf8",
      ),
    );
    return plainObject(parsed) &&
      plainObject(parsed.reportFirstLines) &&
      Array.isArray(parsed.reportMarkers)
      ? parsed
      : undefined;
  } catch {
    return undefined;
  }
};

const delegateTaskFindings = (role, args, actor, root, branch, project) => {
  if (role !== ROLE.implementer) {
    const detail =
      actor?.agent_id === "antigravity-delegated-child"
        ? "하위 스레드(delegated)에서는 하위 에이전트를 위임하지 않는다"
        : "리뷰어·비판자·주 작업 트리·미확인 세션에서는 하위 에이전트를 위임하지 않는다";
    return [
      finding(
        "role-without-delegation",
        `${detail} (docs/workflow.md 1장, docs/harness.md 7.4)`,
      ),
    ];
  }
  if (
    !plainObject(args) ||
    !Object.keys(args).every((key) => DELEGATE_ALLOWED_KEYS.includes(key))
  ) {
    return [finding("orchestration", "허용하지 않은 T3 위임 인자다")];
  }
  if (
    (args.runtimeMode !== undefined &&
      !["inherit", "approval-required"].includes(args.runtimeMode)) ||
    (args.interactionMode !== undefined &&
      !["default", "plan"].includes(args.interactionMode))
  ) {
    return [finding("orchestration", "권한 확대는 허용하지 않는다")];
  }
  if (
    args.isolation !== "worktree" ||
    (args.mode !== undefined && !["wait", "async"].includes(args.mode)) ||
    typeof args.task !== "string" ||
    args.task.trim().length === 0
  ) {
    return [
      finding(
        "orchestration",
        "구현 담당의 위임은 isolation=worktree와 비어 있지 않은 task를 요구한다",
      ),
    ];
  }
  if (!["review", "research"].includes(args.role)) {
    return [
      finding(
        "orchestration",
        "구현 담당은 review·research 역할만 위임할 수 있으며 implementation 등 구현 위임은 거부한다",
      ),
    ];
  }
  const targetFindings = delegateTargetFindings(args.target, args.role);
  if (targetFindings.length > 0) {
    return targetFindings;
  }
  const agentRules = readAgentRules(project.root);
  if (agentRules === undefined) {
    return [
      finding(
        "orchestration",
        "주 작업 트리의 agent-rules.json을 읽을 수 없어 위임을 닫힌 실패로 거부한다",
      ),
    ];
  }
  const report = reportProblems(args.task, agentRules, new Set());
  if (report.length > 0) {
    return report;
  }
  if (args.role === "review") {
    const brief = parseBrief(args.task);
    const base =
      brief.problem === undefined
        ? resolveCommit(root, brief.commit)
        : undefined;
    const head = headOf(root);
    const requested = reviewTargetCommit(args.task);
    const targetCommit =
      requested === undefined ? undefined : resolveCommit(root, requested);
    if (
      brief.problem !== undefined ||
      typeof branch !== "string" ||
      brief.branch !== branch ||
      base === undefined ||
      head === undefined ||
      targetCommit === undefined ||
      base === targetCommit ||
      targetCommit !== head ||
      !isAncestor(root, base, targetCommit)
    ) {
      return [
        finding(
          "orchestration",
          "리뷰 기준은 현재 작업 사본 브랜치와 이력의 기준·대상 커밋(HEAD)이어야 한다",
        ),
      ];
    }
  }
  return [];
};

export const antigravityPreTool = (
  input,
  project,
  sessionCwd = process.cwd(),
  extraFacts = {},
) => {
  try {
    const { root, role, branch, actor } = actorInput(
      input,
      project,
      sessionCwd,
      extraFacts,
    );
    if (!plainObject(input.toolCall)) {
      return refusal("toolCall 객체가 없다");
    }
    const toolName = input.toolCall.name;
    const args = input.toolCall.args;
    if (typeof toolName !== "string" || toolName.length === 0) {
      return refusal("toolCall.name이 비어 있다");
    }
    if (!plainObject(args)) {
      return refusal("toolCall.args가 객체가 아니다");
    }
    const cwd = toolDirectory(
      { cwd: input.cwd ?? root, tool_input: normalizedToolArgs(args) },
      root,
      sessionCwd,
    );
    const home = extraFacts.home ?? homedir();
    const policy =
      extraFacts.policy ?? loadAntigravityPolicy(project.root, project);
    const scoped = { ...project, ownedWorktree: root };

    if (WRITE_TOOLS.includes(toolName)) {
      const extracted = writePathsFromArgs(toolName, args);
      if (extracted.problem !== undefined) return refusal(extracted.problem);
      const allFindings = [];
      for (const target of extracted.paths) {
        const homeDenied = homeWriteFindings(target, policy, home);
        if (homeDenied.length > 0) {
          allFindings.push(...homeDenied);
          continue;
        }
        const expanded = expandHome(target, home);
        if (expanded.path !== undefined) {
          const resolved = isAbsolute(expanded.path)
            ? resolve(expanded.path)
            : resolve(cwd, expanded.path);
          const resolvedHomeDenied = homeWriteFindings(resolved, policy, home);
          if (resolvedHomeDenied.length > 0) {
            allFindings.push(...resolvedHomeDenied);
            continue;
          }
        }
        const shape = shapeProblem(target);
        const rawEditFindings = editFindings(
          { ...actor, tool_input: { file_path: target } },
          scoped,
        );
        const located =
          shape === undefined ? locate(target, scoped.worktrees) : {};
        const extraPathFindings =
          located.relative === undefined
            ? []
            : antigravityPathFindings(located.relative);
        const adjusted = rawEditFindings.map((item) =>
          project.lock === LOCK.off &&
          role === ROLE.implementer &&
          item.rule === "main-only-path" &&
          located.relative !== undefined &&
          REVIEW_RECORD_PATTERN.test(located.relative)
            ? { ...item, gated: true }
            : item,
        );
        allFindings.push(...extraPathFindings, ...adjusted);
      }
      const verdict = decide(allFindings, project.lock);
      return verdict.decision === DECISION.deny
        ? denyOutput(verdict.findings)
        : { decision: "allow" };
    }

    if (READ_TOOLS.includes(toolName)) {
      const extracted = readPathsFromArgs(toolName, args, cwd);
      if (extracted.problem !== undefined) return refusal(extracted.problem);
      const findings = extracted.paths.flatMap((path) =>
        readTargetFindings(path, cwd, policy, root, home),
      );
      return findings.length > 0 ? denyOutput(findings) : { decision: "allow" };
    }

    if (SHELL_TOOLS.includes(toolName)) {
      const command = commandFromArgs(args);
      const rawBash = bashFindings(
        { ...actor, cwd, tool_input: { ...args, command } },
        scoped,
      );
      const adjustedBash = rawBash.map((item) =>
        project.lock === LOCK.off &&
        role === ROLE.implementer &&
        item.rule === "harness-write-mode" &&
        isStaticPrelockGenerator(command)
          ? { ...item, gated: true }
          : item,
      );
      const patches = shellPatches(command).flatMap((patch) =>
        patchFindings(patch, cwd, actor, scoped, root, policy, home),
      );
      const extraWrites = shellWriteTargets(
        command,
        cwd,
        scoped,
        role,
        policy,
        home,
      );
      const gitMutations = gitProtectedMutationFindings(
        command,
        cwd,
        scoped,
        actor,
        role,
      );
      const reviewerGit = reviewerGitFindings(command, role, project.rules);
      const lockedGenerators = lockedConfigGeneratorFindings(
        command,
        project.lock,
        project.rules,
        cwd,
        scoped.worktrees,
      );
      const readAndToss = analyzeScriptTossAndReads(command, {
        cwd,
        policy,
        ownedWorktree: root,
        home,
        rules: project.rules,
      });
      const verdict = decide(
        [
          ...adjustedBash,
          ...patches,
          ...extraWrites,
          ...gitMutations,
          ...reviewerGit,
          ...lockedGenerators,
          ...readAndToss,
        ],
        project.lock,
      );
      return verdict.decision === DECISION.deny
        ? denyOutput(verdict.findings)
        : { decision: "allow" };
    }

    if (DELEGATE_TASK_TOOLS.includes(toolName)) {
      const findings = delegateTaskFindings(
        role,
        args,
        actor,
        root,
        branch,
        project,
      );
      return findings.length > 0 ? denyOutput(findings) : { decision: "allow" };
    }

    if (toolName === "manage_task") {
      const allowed =
        role === ROLE.reviewer
          ? ["list", "status"]
          : ["list", "status", "kill"];
      return allowed.includes(args.Action)
        ? { decision: "allow" }
        : denyOutput([
            finding(
              "denied-tool-action",
              `manage_task의 ${String(args.Action)} 동작은 허용하지 않는다`,
            ),
          ]);
    }

    if (
      role === ROLE.reviewer
        ? SAFE_REVIEWER_TOOLS.includes(toolName)
        : role !== ROLE.unknown && SAFE_IMPLEMENTER_TOOLS.includes(toolName)
    ) {
      return { decision: "allow" };
    }

    return denyOutput([
      finding("denied-tool", `허용되지 않은 도구: ${toolName}`),
    ]);
  } catch (error) {
    return refusal(error.message);
  }
};

export const antigravityPostTool = () => ({});

export const antigravityStop = (input, session, facts = {}) => {
  try {
    const { root, role } = actorInput(
      input,
      session.project,
      session.root,
      facts,
    );
    if (
      input.terminationReason !== undefined &&
      input.terminationReason !== "NO_TOOL_CALL"
    ) {
      return { decision: "allow" };
    }
    if (input.fullyIdle === false) {
      return { decision: "allow" };
    }
    if (typeof input.executionNum === "number" && input.executionNum > 8) {
      return { decision: "allow" };
    }
    if (role === ROLE.reviewer || role === ROLE.unknown) {
      const rawTracked =
        facts.trackedDirty !== undefined
          ? facts.trackedDirty
          : git(root, ["status", "--porcelain=v1", "-uno"]);
      if (typeof rawTracked !== "string") {
        return {
          decision: "continue",
          reason:
            "git status 실행에 실패해 리뷰어 작업 사본 상태를 확인할 수 없다 (닫힌 실패).",
        };
      }
      if (rawTracked.trim() !== "") {
        return {
          decision: "continue",
          reason:
            "리뷰어·비판자 작업 사본에 추적 파일 변경이 남아 있다. 변경을 되돌린 뒤 보고하라.",
        };
      }
      return { decision: "allow" };
    }
    if (role === ROLE.implementer) {
      const hooksReady = facts.hooksReady ?? true;
      if (!hooksReady) {
        return {
          decision: "continue",
          reason:
            "Git 커밋 훅(lefthook)이 설치되어 있지 않다. 훅 상태를 확인한 뒤 보고하라.",
        };
      }
      const rawDirty =
        facts.dirty !== undefined
          ? facts.dirty
          : git(root, ["status", "--porcelain=v1", "-uall"]);
      if (typeof rawDirty !== "string") {
        return {
          decision: "continue",
          reason:
            "git status 실행에 실패해 작업 사본 상태를 확인할 수 없다 (닫힌 실패).",
        };
      }
      const blocked =
        facts.blockedExit !== undefined ? facts.blockedExit : blockedExit(root);
      if (rawDirty.trim() !== "" && blocked === true) {
        return { decision: "allow" };
      }
      if (rawDirty.trim() !== "") {
        return {
          decision: "continue",
          reason:
            '커밋되지 않은 변경이 작업 사본에 남아 있다. 변경을 커밋(정상 Git 커밋 훅 포함)하거나 정리한 뒤 종료하라. 고칠 수 없어 멈춘다면 `node harness/antigravity-hooks/staged-sync.mjs blocked <한 줄 이유>`로 막힘을 기록한 뒤 "막힘:"으로 보고하라.',
        };
      }
    }
    return { decision: "allow" };
  } catch (error) {
    return {
      decision: "continue",
      reason: `Antigravity 하네스 종료 훅 오류: ${error.message}`,
    };
  }
};
