import { statSync } from "node:fs";
import { homedir } from "node:os";
import { basename, isAbsolute, resolve } from "node:path";
import {
  DEFINITIONS,
  WRITERS,
  bashFindings,
  unwrap,
} from "../../.claude/hooks/lib/bash-guard.mjs";
import { editFindings } from "../../.claude/hooks/lib/edit-guard.mjs";
import { delegateTargetFindings } from "../../.claude/hooks/lib/orchestration.mjs";
import {
  locate,
  realTarget,
  shapeProblem,
} from "../../.claude/hooks/lib/paths.mjs";
import { LOCK, git } from "../../.claude/hooks/lib/project.mjs";
import { ROLE } from "../../.claude/hooks/lib/roles.mjs";
import { parseScript } from "../../.claude/hooks/lib/shell.mjs";
import { DECISION, decide } from "../../.claude/hooks/lib/verdict.mjs";
import {
  patchPaths,
  shellPatches,
  toolDirectory,
} from "../codex-hooks/adapter.mjs";
import {
  KEYCHAIN_DENIED_SUBCOMMANDS,
  LOOPBACK_HOSTS,
  NETWORK_CLIENTS,
  TOSS_LIVE_BINARIES,
  antigravityPathFindings,
  expandHome,
  homeWriteFindings,
  loadAntigravityPolicy,
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
]);

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
]);

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
  "-K",
  "--config",
  "--netrc-file",
  "-w",
  "--write-out",
]);

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

const isReviewModelName = (modelName) =>
  typeof modelName === "string" && /-(?:high|medium|low)$/iu.test(modelName);

const isReviewBranch = (branch) =>
  branch === undefined ||
  branch === "" ||
  ["master", "main"].includes(branch.toLowerCase()) ||
  /^(?:review|critic)\//iu.test(branch);

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
      role: ROLE.main,
      branch: undefined,
      actor: {},
    };
  }
  const branch = Object.hasOwn(extraFacts, "branch")
    ? extraFacts.branch
    : currentBranch(located.root);
  const reviewer =
    isReviewBranch(branch) ||
    isReviewModelName(input.modelName) ||
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

const writePathFromArgs = (toolName, args) => {
  if (!plainObject(args)) return undefined;
  if (toolName === "NotebookEdit") return args.notebook_path ?? args.Path;
  return args.Path ?? args.TargetFile ?? args.file_path ?? args.path;
};

const readPathsFromArgs = (toolName, args, fallbackCwd) => {
  if (!plainObject(args)) {
    return { problem: "도구 인자가 객체가 아니다" };
  }
  const candidates = [
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
  if (text.startsWith("@") && text.length > 1) return [text.slice(1)];
  if (!text.startsWith("-")) return [text];
  const equals = text.indexOf("=");
  if (equals === -1) return [];
  const value = text.slice(equals + 1);
  return value.startsWith("@") && value.length > 1 ? [value.slice(1)] : [value];
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
  while (index < args.length) {
    const text = args[index].text;
    if (!text.startsWith("-")) {
      return { sub: text, rest: args.slice(index + 1) };
    }
    const equals = text.indexOf("=");
    const flag = equals === -1 ? text : text.slice(0, equals);
    index += GIT_VALUED_GLOBALS.has(flag) && equals === -1 ? 2 : 1;
  }
  return { sub: undefined, rest: [] };
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
  return scripts;
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
    const text = args[index].text;
    if (text === "--") break;
    if (["-p", "--package", "--bin", "--manifest-path"].includes(text)) {
      const next = args[index + 1]?.text;
      if (next !== undefined) targets.push({ flag: text, value: next });
      index += 1;
    } else if (
      text.startsWith("--package=") ||
      text.startsWith("--bin=") ||
      text.startsWith("--manifest-path=")
    ) {
      const sep = text.indexOf("=");
      targets.push({ flag: text.slice(0, sep), value: text.slice(sep + 1) });
    }
  }
  return targets;
};

const cargoSubcommand = (args) =>
  args.find((word) => !word.text.startsWith("-") && !word.text.startsWith("+"))
    ?.text;

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
      } else if (valueText && looksLikePath(valueText)) {
        checkReadCandidate(valueText, cwd);
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
        const name = word.text.split(/\+?=/u)[0];
        if (/^TOSS_/iu.test(name)) {
          findings.push(
            finding(
              "toss-live-api",
              `실제 토스 환경 변수(${name})를 내보내지 않는다`,
            ),
          );
        }
      }
    }
    for (const redir of command.redirects) {
      if (["<", "<>", "<&"].includes(redir.operator)) {
        checkReadCandidate(
          redir.target.text,
          invocation.cwdUnknown ? undefined : cwd,
        );
      }
    }
    const name = invocation.name;
    const effectiveCwd = invocation.cwdUnknown ? undefined : cwd;
    if (name === "security") {
      const sub = invocation.args.find(
        (word) => !word.text.startsWith("-"),
      )?.text;
      if (sub === undefined || KEYCHAIN_DENIED_SUBCOMMANDS.includes(sub)) {
        findings.push(
          finding(
            "protected-home-read",
            `macOS 키체인 조회 명령(security ${sub ?? ""})은 허용하지 않는다`,
          ),
        );
      }
    }
    if (name !== undefined && TOSS_LIVE_BINARIES.includes(name)) {
      findings.push(
        finding(
          "toss-live-api",
          `실제 API·앱 실행 바이너리(${name})는 에이전트가 직접 실행하지 않는다`,
        ),
      );
    }
    if (name === "cargo") {
      const sub = cargoSubcommand(invocation.args);
      if (sub === "run") {
        const targets = cargoPackageOrBinTargets(invocation.args);
        const packages = targets
          .filter((item) => ["-p", "--package"].includes(item.flag))
          .map((item) => item.value);
        const bins = targets
          .filter((item) => item.flag === "--bin")
          .map((item) => item.value);
        const manifests = targets
          .filter((item) => item.flag === "--manifest-path")
          .map((item) => item.value);
        if (
          packages.some((pkg) =>
            ["toss-smoke", "broker-toss-trade"].includes(pkg),
          ) ||
          bins.some((bin) =>
            ["toss-smoke", "broker-toss-trade"].includes(bin),
          ) ||
          manifests.some(
            (manifest) =>
              manifest.includes("toss-smoke") ||
              manifest.includes("broker-toss-trade"),
          )
        ) {
          findings.push(
            finding(
              "toss-live-api",
              "실제 토스 API·주문 crate(toss-smoke, broker-toss-trade)는 에이전트가 실행하지 않는다",
            ),
          );
        }
        const runsTauri =
          packages.includes("src-tauri") ||
          bins.includes("src-tauri") ||
          manifests.some((manifest) => manifest.includes("src-tauri"));
        if (runsTauri && !bins.includes("gen-bindings")) {
          findings.push(
            finding(
              "toss-live-api",
              "실제 앱 바이너리(src-tauri)는 gen-bindings 외에 에이전트가 직접 실행하지 않는다",
            ),
          );
        }
      }
      if (sub === "tauri") {
        const nextSub = invocation.args
          .filter((word) => !word.text.startsWith("-"))
          .at(1)?.text;
        if (nextSub === "dev") {
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
    if (name !== undefined && NETWORK_CLIENTS.includes(name)) {
      if (
        name === "curl" &&
        invocation.args.some(
          (word) =>
            word.text === "-n" ||
            word.text === "--netrc" ||
            word.text === "--netrc-optional",
        )
      ) {
        findings.push(
          finding(
            "protected-home-read",
            "curl의 netrc 읽기 옵션(-n/--netrc)은 허용하지 않는다",
          ),
        );
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
          if (HTTP_CLIENTS.has(name) && CURL_VALUED_FLAGS.has(word.text)) {
            index += 1;
            continue;
          }
          if (word.text.startsWith("-")) continue;
          const host =
            hostFromWord(word.text) ??
            (HTTP_CLIENTS.has(name) ? bareHostFromWord(word.text) : undefined);
          if (
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
        if (
          [
            "nc",
            "ncat",
            "socat",
            "telnet",
            "ssh",
            "scp",
            "sftp",
            "ftp",
          ].includes(name)
        ) {
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
          invocation.args.some((word) => word.text === "s_client")
        ) {
          findings.push(
            finding(
              "toss-live-api",
              "openssl s_client 외부 연결은 허용하지 않는다",
            ),
          );
        }
      }
    }
    const isPureEcho = ["echo", "printf"].includes(name ?? "");
    const isReader =
      name !== undefined &&
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
      for (const candidate of [absolute, realTarget(absolute)]) {
        const located = locate(candidate, project.worktrees);
        if (located.root !== undefined && located.relative !== undefined) {
          findings.push(...antigravityPathFindings(located.relative, role));
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
        const isDetach =
          sub === "checkout" &&
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

const lockedConfigGeneratorFindings = (source, lock, rules, depth = 0) => {
  if (lock === LOCK.off || depth > 8) return [];
  const parsed = parseScript(source);
  const findings = [];
  for (const command of parsed.commands) {
    const invocation = unwrap(command, DEFINITIONS, rules);
    if (invocation.name === "node") {
      const hasWrite = invocation.args.some(
        (word) => word.dynamic || word.text === "--write",
      );
      if (hasWrite) {
        const hit = invocation.args.find((word) => {
          const normalized = word.text.replace(/^(?:\.\/)+/u, "");
          return (
            LOCKED_CONFIG_GENERATORS.has(normalized) ||
            [...LOCKED_CONFIG_GENERATORS].some((target) =>
              normalized.endsWith(`/${target}`),
            )
          );
        });
        if (hit !== undefined) {
          findings.push(
            finding(
              "harness-write-mode",
              `${hit.text} --write는 잠금 on에서 허용하지 않는다`,
            ),
          );
        }
      }
    }
    for (const nested of shellNestedScripts(invocation, rules)) {
      findings.push(
        ...lockedConfigGeneratorFindings(nested, lock, rules, depth + 1),
      );
    }
  }
  for (const substitution of parsed.substitutions) {
    findings.push(
      ...lockedConfigGeneratorFindings(substitution, lock, rules, depth + 1),
    );
  }
  return findings;
};

const patchFindings = (patch, cwd, actor, role, project, owned, policy, home) =>
  patchPaths(patch).flatMap((path) => {
    const homeDenied = homeWriteFindings(path, policy, home);
    if (homeDenied.length > 0) return homeDenied;
    const absolute = resolve(cwd, path);
    if (!absolute.startsWith(`${owned}/`)) {
      throw new Error("세션 작업 사본 밖의 편집이다");
    }
    const located = locate(absolute, project.worktrees);
    const extra =
      located.relative === undefined
        ? []
        : antigravityPathFindings(located.relative, role);
    return [
      ...extra,
      ...editFindings(
        { ...actor, tool_input: { file_path: absolute } },
        project,
      ),
    ];
  });

const delegateTaskFindings = (args, role) => {
  if (role === ROLE.reviewer || role === ROLE.unknown) {
    return [
      finding(
        "role-without-delegation",
        "리뷰어·비판자는 하위 에이전트를 위임하지 않는다",
      ),
    ];
  }
  if (!plainObject(args)) {
    return [finding("orchestration", "delegate_task 인자가 객체가 아니다")];
  }
  const allowedKeys = [
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
  ];
  if (!Object.keys(args).every((key) => allowedKeys.includes(key))) {
    return [finding("orchestration", "허용하지 않은 delegate_task 인자다")];
  }
  if (
    args.runtimeMode !== undefined &&
    !["inherit", "approval-required"].includes(args.runtimeMode)
  ) {
    return [finding("orchestration", "권한 확대는 허용하지 않는다")];
  }
  if (
    args.interactionMode !== undefined &&
    !["default", "plan"].includes(args.interactionMode)
  ) {
    return [finding("orchestration", "허용하지 않은 interactionMode다")];
  }
  if (
    args.isolation !== "worktree" ||
    (args.mode !== undefined && !["wait", "async"].includes(args.mode)) ||
    typeof args.task !== "string" ||
    args.task.trim() === ""
  ) {
    return [finding("orchestration", "worktree 격리와 작업 문구가 필요하다")];
  }
  return delegateTargetFindings(args.target, args.role);
};

export const antigravityPreTool = (
  input,
  project,
  sessionCwd = process.cwd(),
  extraFacts = {},
) => {
  try {
    const { root, role, actor } = actorInput(
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
      const target = writePathFromArgs(toolName, args);
      if (typeof target !== "string" || target.length === 0) {
        return refusal(`${toolName}의 대상 경로가 없다`);
      }
      const homeDenied = homeWriteFindings(target, policy, home);
      if (homeDenied.length > 0) return denyOutput(homeDenied);
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
          : antigravityPathFindings(located.relative, role);
      const adjusted = rawEditFindings.map((item) =>
        project.lock === LOCK.off &&
        role === ROLE.implementer &&
        item.rule === "main-only-path" &&
        located.relative !== undefined &&
        REVIEW_RECORD_PATTERN.test(located.relative)
          ? { ...item, gated: true }
          : item,
      );
      const verdict = decide([...extraPathFindings, ...adjusted], project.lock);
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
        patchFindings(patch, cwd, actor, role, scoped, root, policy, home),
      );
      const extraWrites = shellWriteTargets(
        command,
        cwd,
        scoped,
        role,
        policy,
        home,
      );
      const reviewerGit = reviewerGitFindings(command, role, project.rules);
      const lockedGenerators = lockedConfigGeneratorFindings(
        command,
        project.lock,
        project.rules,
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

    if (toolName === "mcp_t3-code_delegate_task") {
      const findings = delegateTaskFindings(args, role);
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
    if (input.fullyIdle === false || input.continuationActive === true) {
      return { decision: "allow" };
    }
    if (role === ROLE.reviewer || role === ROLE.unknown) {
      const trackedDirty =
        facts.trackedDirty ??
        git(root, ["status", "--porcelain=v1", "-uno"])?.trim() ??
        "";
      if (trackedDirty !== "") {
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
      const dirty =
        facts.dirty ??
        git(root, ["status", "--porcelain=v1", "-uall"])?.trim() ??
        "";
      if (dirty !== "") {
        return {
          decision: "continue",
          reason:
            "커밋되지 않은 변경이 작업 사본에 남아 있다. 변경을 커밋(정상 Git 커밋 훅 포함)하거나 정리한 뒤 종료하라.",
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
