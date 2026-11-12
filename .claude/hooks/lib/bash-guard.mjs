import { execFileSync } from "node:child_process";
import { realpathSync, statSync } from "node:fs";
import { homedir } from "node:os";
import { basename, isAbsolute, join, resolve } from "node:path";
import {
  MODE,
  TEMPORARY_ROOTS,
  locate,
  realTarget,
  writeFindings,
} from "./paths.mjs";
import { gitEnvironment } from "./project.mjs";
import { ROLE, isSubagent, roleOf } from "./roles.mjs";
import { parseScript } from "./shell.mjs";
import { decide } from "./verdict.mjs";

const MAX_DEPTH = 8;

const finding = (rule, message, gated = false) => ({ rule, gated, message });

const remembered = (compute) => {
  const answers = new Map();
  return (...inputs) => {
    const key = inputs.join("\0");
    if (!answers.has(key)) {
      answers.set(key, compute(...inputs));
    }
    return answers.get(key);
  };
};

const literal = (text) => ({
  text,
  dynamic: false,
  quoted: false,
  glob: false,
  raw: text,
});

const commandName = (word) => basename(word.text).toLowerCase();

const isOption = (word) =>
  !word.dynamic && word.text.startsWith("-") && word.text !== "-";

const quoteWord = (word) => `'${word.text.replaceAll("'", "'\\''")}'`;

const skipOptions = (args, valued) => {
  let index = 0;
  while (index < args.length) {
    const word = args[index];
    if (word.text === "--" && !word.dynamic) {
      return index + 1;
    }
    if (!isOption(word)) {
      return index;
    }
    index += valued.includes(word.text) ? 2 : 1;
  }
  return index;
};

const assignmentOf = (word) => {
  const match = /^([A-Za-z_][A-Za-z0-9_]*)=/u.exec(word.text);
  return match === null || word.dynamic
    ? undefined
    : { name: match[1], value: literal(word.text.slice(match[0].length)) };
};

const ENV_FLAGS = new Set(["i", "0", "v"]);
const ENV_VALUED = new Set(["u", "C", "P", "S"]);
const ENV_LONG_FLAGS = new Set(["--ignore-environment", "--null", "--debug"]);
const ENV_LONG_VALUED = new Map([
  ["--unset", "u"],
  ["--chdir", "C"],
  ["--split-string", "S"],
]);

const envOption = (args, index) => {
  const text = args[index].text;
  if (text === "-") {
    return { options: [], next: index + 1 };
  }
  if (text.startsWith("--")) {
    const [name, attached] = text.includes("=")
      ? [text.slice(0, text.indexOf("=")), text.slice(text.indexOf("=") + 1)]
      : [text, undefined];
    if (ENV_LONG_FLAGS.has(name)) {
      return { options: [], next: index + 1 };
    }
    const letter = ENV_LONG_VALUED.get(name);
    if (letter === undefined) {
      return { problem: `env의 옵션 ${text}를 모른다` };
    }
    return attached === undefined
      ? { options: [{ letter, value: args[index + 1] }], next: index + 2 }
      : { options: [{ letter, value: literal(attached) }], next: index + 1 };
  }
  const cluster = text.slice(1);
  for (let position = 0; position < cluster.length; position += 1) {
    const letter = cluster[position];
    if (ENV_VALUED.has(letter)) {
      const rest = cluster.slice(position + 1);
      return rest === ""
        ? { options: [{ letter, value: args[index + 1] }], next: index + 2 }
        : { options: [{ letter, value: literal(rest) }], next: index + 1 };
    }
    if (!ENV_FLAGS.has(letter)) {
      return { problem: `env의 옵션 묶음 ${text}를 모른다` };
    }
  }
  return { options: [], next: index + 1 };
};

const unwrapEnv = (args, invocation) => {
  let index = 0;
  while (index < args.length) {
    const word = args[index];
    if (word.dynamic) {
      return { problem: "env 인자가 동적이다" };
    }
    if (word.text === "--") {
      index += 1;
      continue;
    }
    if (word.text.startsWith("-")) {
      const parsed = envOption(args, index);
      if (parsed.problem !== undefined) {
        return parsed;
      }
      for (const option of parsed.options) {
        if (option.value === undefined || option.value.dynamic) {
          return { problem: `env -${option.letter}의 값이 없거나 동적이다` };
        }
        if (option.letter === "C") {
          invocation.cwdUnknown = true;
        }
        if (option.letter === "S") {
          const rest = args.slice(parsed.next).map(quoteWord).join(" ");
          return { script: `${option.value.text} ${rest}`.trim() };
        }
      }
      index = parsed.next;
      continue;
    }
    const assignment = assignmentOf(word);
    if (assignment === undefined) {
      break;
    }
    invocation.env.push(assignment);
    index += 1;
  }
  return { args: args.slice(index) };
};

const unwrapArch = (args, invocation) => {
  let index = 0;
  while (index < args.length && isOption(args[index])) {
    if (args[index].text === "-e") {
      const assignment = assignmentOf(args[index + 1] ?? literal(""));
      if (assignment !== undefined) {
        invocation.env.push(assignment);
      }
      index += 2;
    } else {
      index += 1;
    }
  }
  return { args: args.slice(index) };
};

const unwrapTimeout = (args) => {
  const start = skipOptions(args, ["-s", "-k", "--signal", "--kill-after"]);
  return { args: args.slice(start + 1) };
};

const unwrapCommand = (args, invocation) => {
  const start = skipOptions(args, []);
  if (args.slice(0, start).some((word) => /^-[a-zA-Z]*[vV]/u.test(word.text))) {
    invocation.executes = false;
  }
  return { args: args.slice(start) };
};

const unwrapScript = (args) => {
  const start = skipOptions(args, ["-F", "-t"]);
  return args.length > start + 1
    ? { args: args.slice(start + 1) }
    : { self: true };
};

const unwrapPackageRunner = (args) => {
  const start = skipOptions(args, ["-p", "--package"]);
  const options = args.slice(0, start).map((word) => word.text);
  if (options.some((text) => ["-c", "--call", "--shell-mode"].includes(text))) {
    return { problem: "패키지 실행기에 셸 문자열을 넘기지 않는다" };
  }
  return { args: args.slice(start) };
};

const unwrapPnpm = (args) => {
  const start = skipOptions(args, ["-C", "--dir", "--filter", "-F"]);
  const sub = args[start];
  if (sub === undefined || sub.dynamic) {
    return sub === undefined
      ? { self: true }
      : { problem: "pnpm 하위 명령이 동적이다" };
  }
  return ["exec", "dlx"].includes(sub.text)
    ? unwrapPackageRunner(args.slice(start + 1))
    : { self: true };
};

const unwrapNpm = (args) => {
  const start = skipOptions(args, ["-w", "--workspace", "--prefix"]);
  const sub = args[start];
  return sub !== undefined && !sub.dynamic && sub.text === "exec"
    ? unwrapPackageRunner(args.slice(start + 1))
    : { self: true };
};

const simple = (valued) => (args) => ({
  args: args.slice(skipOptions(args, valued)),
});

export const WRAPPERS = {
  command: unwrapCommand,
  builtin: simple([]),
  exec: simple(["-a"]),
  nohup: simple([]),
  nice: simple(["-n"]),
  caffeinate: simple(["-t", "-w"]),
  stdbuf: simple(["-i", "-o", "-e"]),
  sudo: simple(["-u", "-g", "-h", "-p", "-C", "-D", "-r", "-t", "-U"]),
  doas: simple(["-u", "-C"]),
  xcrun: simple(["--sdk", "--toolchain"]),
  setsid: simple([]),
  timeout: unwrapTimeout,
  gtimeout: unwrapTimeout,
  arch: unwrapArch,
  env: unwrapEnv,
  script: unwrapScript,
  npx: unwrapPackageRunner,
  pnpm: unwrapPnpm,
  npm: unwrapNpm,
  xargs: (args, invocation) => {
    invocation.hidden = true;
    return {
      args: args.slice(
        skipOptions(args, ["-I", "-L", "-n", "-P", "-s", "-E", "-d", "-a"]),
      ),
    };
  },
};

export const RESERVED_PREFIXES = [
  "!",
  "{",
  "if",
  "then",
  "else",
  "elif",
  "do",
  "while",
  "until",
  "time",
  "coproc",
];

const watchedName = (word, rules) =>
  !word.dynamic && rules.watched.includes(commandName(word));

export const unwrap = (command, definitions, rules) => {
  const invocation = {
    name: undefined,
    args: [],
    env: [...command.assignments],
    hidden: false,
    executes: true,
    command,
    problems: [],
    cwdUnknown: false,
    shadows: undefined,
    script: undefined,
  };
  let words = command.words;
  for (let guard = 0; guard < 32 && words.length > 0; guard += 1) {
    const head = words[0];
    if (head.dynamic || (head.glob && head.text !== "[")) {
      invocation.problems.push(`명령 이름이 정해지지 않았다: ${head.raw}`);
      return invocation;
    }
    const name = commandName(head);
    if (definitions.reserved.includes(name) && !head.quoted) {
      words = words.slice(1);
      continue;
    }
    if (name === "function" && !head.quoted) {
      if (words[1] !== undefined && watchedName(words[1], rules)) {
        invocation.shadows = commandName(words[1]);
      }
      words = words.slice(2);
      continue;
    }
    if (command.defines === true && words.length === 1) {
      invocation.shadows = watchedName(head, rules) ? name : undefined;
      invocation.executes = false;
      return invocation;
    }
    const wrapper = Object.hasOwn(definitions.wrappers, name)
      ? definitions.wrappers[name]
      : undefined;
    const unwrapped =
      wrapper === undefined
        ? { self: true }
        : wrapper(words.slice(1), invocation);
    if (unwrapped.self === true) {
      invocation.name = name;
      invocation.args = words.slice(1);
      return invocation;
    }
    if (unwrapped.problem !== undefined) {
      invocation.problems.push(unwrapped.problem);
      return invocation;
    }
    if (unwrapped.script !== undefined) {
      invocation.script = unwrapped.script;
      return invocation;
    }
    if (!invocation.executes) {
      return invocation;
    }
    if (unwrapped.args.length === 0) {
      invocation.name = name;
      return invocation;
    }
    words = unwrapped.args;
  }
  return invocation;
};

const environmentFindings = (names, rules, rule) =>
  names
    .filter(
      (name) =>
        rules.environment.names.includes(name) ||
        rules.environment.prefixes.some((prefix) => name.startsWith(prefix)),
    )
    .map((name) =>
      finding(
        rule,
        `${name} 환경 변수로 훅·git·Rust·Node 검사나 설정 위치를 바꾸지 않는다 (docs/harness.md 7.3)`,
      ),
    );

const GIT_VALUED = [
  "-C",
  "-c",
  "--git-dir",
  "--work-tree",
  "--namespace",
  "--config-env",
  "--super-prefix",
  "--attr-source",
];

const parseGit = (args) => {
  const globals = [];
  let index = 0;
  while (index < args.length) {
    const word = args[index];
    if (word.dynamic) {
      return { globals, dynamic: true };
    }
    const text = word.text;
    if (!text.startsWith("-")) {
      return { globals, sub: text, rest: args.slice(index + 1), index };
    }
    const [option, attached] = text.includes("=")
      ? [text.slice(0, text.indexOf("=")), text.slice(text.indexOf("=") + 1)]
      : [text, undefined];
    if (GIT_VALUED.includes(option) && attached === undefined) {
      globals.push({ option, value: args[index + 1] });
      index += 2;
    } else {
      globals.push({
        option,
        value: attached === undefined ? undefined : literal(attached),
      });
      index += 1;
    }
  }
  return { globals };
};

const longMatches = (text, forbidden) => {
  const option = text.includes("=") ? text.slice(0, text.indexOf("=")) : text;
  return (
    (option.length >= 4 && forbidden.startsWith(option)) ||
    option.startsWith(`${forbidden}-`)
  );
};

const gitOptionWords = (sub, rest, rules) => {
  const values = rules.git.shortValues[sub] ?? { required: "", attached: "" };
  const options = [];
  const positionals = [];
  let index = 0;
  let ended = false;
  while (index < rest.length) {
    const word = rest[index];
    index += 1;
    if (
      ended ||
      word.dynamic ||
      !word.text.startsWith("-") ||
      word.text === "-"
    ) {
      positionals.push({ word, ended });
      continue;
    }
    if (word.text === "--") {
      ended = true;
      continue;
    }
    if (word.text.startsWith("--")) {
      options.push({ long: word.text, text: word.text });
      continue;
    }
    const cluster = word.text.slice(1);
    for (let position = 0; position < cluster.length; position += 1) {
      const character = cluster[position];
      options.push({ short: character, text: word.text });
      if (values.required.includes(character)) {
        if (position === cluster.length - 1) {
          index += 1;
        }
        break;
      }
      if (values.attached.includes(character)) {
        break;
      }
    }
  }
  return { options, positionals };
};

const optionHits = (rule, options) =>
  options.filter(
    (option) =>
      (option.long !== undefined &&
        rule.long.some((forbidden) => longMatches(option.long, forbidden))) ||
      (option.short !== undefined && rule.short.includes(option.short)),
  );

const isAbort = (sub, rest, rules) =>
  rules.git.abortable.includes(sub) &&
  rest.length === 1 &&
  !rest[0].dynamic &&
  rest[0].text === "--abort";

const implementerMerge = (rest, rules) => {
  const allowed = rules.git.implementerMerge;
  const options = rest.filter((word) => isOption(word));
  const positionals = rest.filter((word) => !isOption(word));
  return (
    options.every((word) => allowed.options.includes(word.text)) &&
    positionals.length === 1 &&
    !positionals[0].dynamic &&
    allowed.branches.includes(positionals[0].text)
  );
};

const gitParts = (invocation) => {
  if (invocation.name !== "git") {
    return undefined;
  }
  const parsed = parseGit(invocation.args);
  return parsed.sub === undefined ? undefined : parsed;
};

const aliases = new Map();

const lookupAlias = (sub, cwd) => {
  const key = `${cwd}\0${sub}`;
  if (!aliases.has(key)) {
    try {
      aliases.set(
        key,
        execFileSync("git", ["-C", cwd, "config", "--get", `alias.${sub}`], {
          encoding: "utf8",
          env: gitEnvironment(),
          stdio: ["ignore", "pipe", "ignore"],
        }).trim(),
      );
    } catch {
      aliases.set(key, undefined);
    }
  }
  return aliases.get(key);
};

const aliasDirectory = (parsed, invocation, context) => {
  let directory = invocation.cwdUnknown ? undefined : context.cwd;
  for (const global of parsed.globals.filter((item) => item.option === "-C")) {
    if (global.value === undefined || global.value.dynamic) {
      return undefined;
    }
    directory = isAbsolute(global.value.text)
      ? global.value.text
      : directory === undefined
        ? undefined
        : resolve(directory, global.value.text);
  }
  return directory;
};

const words = (invocation) =>
  invocation.args
    .filter((word) => !word.dynamic)
    .flatMap((word) => word.text.split(/\s+/u))
    .filter((token) => token !== "")
    .map((token) => basename(token).toLowerCase());

const handledNames = (context) => {
  const rules = context.rules;
  return new Set([
    ...rules.readers,
    ...Object.keys(context.definitions.writers),
    ...rules.shells,
    ...Object.keys(rules.interpreters),
    ...rules.awk,
    ...Object.keys(rules.editors),
    ...rules.rust.tools,
    ...rules.environment.exporters,
    ...rules.commands.chmod,
    "git",
    "lefthook",
    "just",
    "pnpm",
    "npm",
    "yarn",
    "rustup",
    "find",
    "eval",
    "alias",
    "trap",
  ]);
};

const interpreterFamily = (name, rules) => {
  if (name === undefined) {
    return undefined;
  }
  const stem = name === "nodejs" ? "node" : name.replace(/[0-9.]+$/u, "");
  return Object.hasOwn(rules.interpreters, stem) ? stem : undefined;
};

const inlineCode = (invocation, flags) => {
  const codes = [];
  const args = invocation.args;
  let scriptGiven = false;
  for (let index = 0; index < args.length; index += 1) {
    const word = args[index];
    const text = word.text;
    const exact = flags.find((flag) => text === flag);
    const clustered =
      exact === undefined &&
      /^-[A-Za-z]+$/u.test(text) &&
      flags.some(
        (flag) => /^-[A-Za-z]$/u.test(flag) && text.endsWith(flag.slice(1)),
      );
    const attached = flags.find(
      (flag) => flag.startsWith("--") && text.startsWith(`${flag}=`),
    );
    if (exact !== undefined || clustered) {
      codes.push(args[index + 1] ?? literal(""));
      index += 1;
    } else if (attached !== undefined) {
      codes.push(literal(text.slice(attached.length + 1)));
    } else if (!isOption(word)) {
      scriptGiven = true;
      break;
    }
  }
  if (codes.length === 0 && !scriptGiven) {
    codes.push(...invocation.command.inputs);
    if (invocation.command.piped && invocation.command.inputs.length === 0) {
      codes.push({ ...literal(""), dynamic: true });
    }
  }
  return codes;
};

const AWK_WRITES =
  /system\s*\(|\|\s*getline|\|\s*["$A-Za-z]|>>|[^><=!]>\s*["$A-Za-z(]/u;

const awkCode = (invocation) => {
  const args = invocation.args;
  const programs = [];
  let index = 0;
  while (index < args.length && isOption(args[index])) {
    if (args[index].text === "-f") {
      return [];
    }
    index += ["-F", "-v"].includes(args[index].text) ? 2 : 1;
  }
  if (args[index] !== undefined) {
    programs.push(args[index]);
  }
  return programs;
};

const SED_COMMAND =
  /(^|[;\n{}])\s*(?:[0-9$]+|\/(?:\\.|[^/])*\/)?(?:,(?:[0-9$]+|\/(?:\\.|[^/])*\/))?\s*!?\s*[wWe](\s|$)/u;
const SED_FLAG = /s(.)(?:\\.|(?!\1).)*\1(?:\\.|(?!\1).)*\1[gpIiMm0-9]*[we]/u;

const sedScripts = (args) => {
  const scripts = args
    .map((word, index) =>
      ["-e", "--expression"].includes(word.text) ? args[index + 1] : undefined,
    )
    .filter((word) => word !== undefined);
  if (scripts.length > 0) {
    return scripts;
  }
  const first = args.find(
    (word) => !isOption(word) && !(word.text === "" && word.quoted),
  );
  return first === undefined ? [] : [first];
};

const keywordFindings = (code, invocation, context) => {
  const lowered = code.text.toLowerCase();
  const rules = context.rules;
  return [
    ...rules.inlineCodeKeywords.filter((keyword) =>
      lowered.includes(keyword.toLowerCase()),
    ),
    ...[...rules.environment.names, ...rules.environment.prefixes].filter(
      (name) => code.text.includes(name),
    ),
  ].map((keyword) =>
    finding(
      "inline-code",
      `${invocation.name} 코드에 하네스가 막는 낱말 ${keyword}가 있다`,
    ),
  );
};

const subagentCode = (invocation) =>
  finding(
    "inline-code",
    `서브에이전트는 ${invocation.name}에 코드를 직접 넘기지 않는다. 쓸 수 있는 경로에 스크립트 파일로 만들어 실행한다`,
  );

const operands = (args, valued = []) => {
  const found = [];
  let ended = false;
  for (let index = 0; index < args.length; index += 1) {
    const word = args[index];
    if (!ended && word.text === "--" && !word.dynamic) {
      ended = true;
    } else if (!ended && isOption(word)) {
      if (valued.includes(word.text)) {
        index += 1;
      }
    } else {
      found.push(word);
    }
  }
  return found;
};

const targetDirectory = (args) => {
  const index = args.findIndex((word) => word.text === "-t");
  const attached = args.find((word) =>
    word.text.startsWith("--target-directory="),
  );
  if (index !== -1 && args[index + 1] !== undefined) {
    return [args[index + 1]];
  }
  return attached === undefined
    ? []
    : [literal(attached.text.slice("--target-directory=".length))];
};

const lastOperand = (args) => {
  const directory = targetDirectory(args);
  if (directory.length > 0) {
    return directory;
  }
  const found = operands(args, ["-t", "-S", "-m", "-o", "-g"]);
  return found.length > 1 ? found.slice(-1) : [];
};

const sedTargets = (args) => {
  const inPlace = args.some(
    (word) =>
      /^-[a-zA-Z]*i/u.test(word.text) || word.text.startsWith("--in-place"),
  );
  if (!inPlace) {
    return [];
  }
  const scripted = args.some((word) =>
    ["-e", "-f", "--expression", "--file"].includes(word.text),
  );
  const found = operands(args, ["-e", "-f", "--expression", "--file"]).filter(
    (word) => !(word.text === "" && word.quoted),
  );
  return scripted ? found : found.slice(1);
};

const perlTargets = (args) => {
  if (!args.some((word) => /^-[a-zA-Z]*i/u.test(word.text))) {
    return [];
  }
  const found = [];
  let coded = false;
  for (let index = 0; index < args.length; index += 1) {
    const word = args[index];
    if (/^-[a-zA-Z]*[eE]$/u.test(word.text)) {
      coded = true;
      index += 1;
    } else if (["-M", "-I"].includes(word.text)) {
      index += 1;
    } else if (!isOption(word)) {
      found.push(word);
    }
  }
  return coded ? found : found.slice(1);
};

export const WRITERS = {
  tee: (args) => operands(args),
  cp: lastOperand,
  ln: lastOperand,
  install: lastOperand,
  rsync: lastOperand,
  ditto: lastOperand,
  mv: (args) => operands(args, ["-t", "-S"]),
  rm: (args) => operands(args),
  rmdir: (args) => operands(args),
  unlink: (args) => operands(args),
  touch: (args) => operands(args, ["-t", "-r", "-d", "-A"]),
  truncate: (args) => operands(args, ["-s", "-r"]),
  shred: (args) => operands(args, ["-n", "-s"]),
  mkdir: (args) => operands(args, ["-m"]),
  sed: sedTargets,
  perl: perlTargets,
  dd: (args) =>
    args
      .filter((word) => word.text.startsWith("of="))
      .map((word) => ({ ...word, text: word.text.slice(3) })),
  curl: (args) =>
    args
      .map((word, index) =>
        ["-o", "--output"].includes(word.text) ? args[index + 1] : undefined,
      )
      .filter((word) => word !== undefined),
  wget: (args) =>
    args
      .map((word, index) =>
        word.text === "-O"
          ? args[index + 1]
          : word.text.startsWith("--output-document=")
            ? literal(word.text.slice("--output-document=".length))
            : undefined,
      )
      .filter((word) => word !== undefined),
};

const writeTargets = (invocation, writers) =>
  invocation.name !== undefined && Object.hasOwn(writers, invocation.name)
    ? writers[invocation.name](invocation.args)
    : [];

const absoluteTarget = (text, cwd) => {
  if (text === "~" || text.startsWith("~/")) {
    return join(homedir(), text.slice(1));
  }
  if (text.startsWith("~")) {
    return undefined;
  }
  if (isAbsolute(text)) {
    return resolve(text);
  }
  return cwd === undefined ? undefined : resolve(cwd, text);
};

const currentDirectory = (invocation, context) =>
  invocation.cwdUnknown ? undefined : context.cwd;

const targetFindings = (target, invocation, context) => {
  if (target.dynamic || target.glob) {
    return [
      finding(
        "dynamic-target",
        `쓰기 대상이 동적이거나 글롭이다: ${target.raw}`,
      ),
    ];
  }
  const lexical = absoluteTarget(
    target.text,
    currentDirectory(invocation, context),
  );
  if (lexical === undefined) {
    return [
      finding(
        "cwd-unknown",
        `현재 폴더를 알 수 없어 상대 경로 쓰기 대상을 풀 수 없다: ${target.raw}`,
      ),
    ];
  }
  return [...new Set([lexical, realTarget(lexical)])].flatMap((path) =>
    writeFindings(path, context.role, context.project, MODE.shell),
  );
};

const scopedTarget = (target, invocation, context) => {
  if (target.dynamic || target.glob) {
    return false;
  }
  const path = absoluteTarget(
    target.text,
    currentDirectory(invocation, context),
  );
  if (path === undefined) {
    return false;
  }
  const real = realTarget(path);
  return [path, real].every((candidate) =>
    locate(candidate, context.project.worktrees).root === undefined
      ? TEMPORARY_ROOTS.some(
          (root) => candidate === root || candidate.startsWith(`${root}/`),
        )
      : writeFindings(candidate, context.role, context.project, MODE.shell)
          .length === 0,
  );
};

const hardLinkCreation = (invocation) => {
  const texts = invocation.args.map((word) => word.text);
  const options = texts.filter((text) => text.startsWith("-") && text !== "-");
  const cluster = (letter) =>
    options.some((text) => /^-[a-zA-Z]+$/u.test(text) && text.includes(letter));
  if (invocation.name === "ln") {
    return !(cluster("s") || options.includes("--symbolic"));
  }
  if (invocation.name === "cp") {
    return cluster("l") || options.includes("--link");
  }
  if (invocation.name === "rsync") {
    return options.some((text) => text.startsWith("--link-dest"));
  }
  return false;
};

const strippedReference = (text) =>
  text
    .toLowerCase()
    .replace(/^\/+/u, "")
    .replace(/^refs\/+/u, "")
    .replace(/^heads\/+/u, "");

const referenceName = remembered((text) => {
  const plain = text.replace(/^\+/u, "");
  if (plain === "") {
    return "";
  }
  try {
    return strippedReference(
      execFileSync(
        "git",
        ["check-ref-format", "--normalize", "--allow-onelevel", plain],
        {
          encoding: "utf8",
          env: gitEnvironment(),
          stdio: ["ignore", "pipe", "ignore"],
        },
      ).trim(),
    );
  } catch {
    return /^[A-Za-z0-9._/-]+$/u.test(plain) && !plain.includes("..")
      ? strippedReference(plain.replace(/\/{2,}/gu, "/"))
      : undefined;
  }
});

const baseOrUnknown = (text, branches) => {
  const name = referenceName(text);
  return (
    name === undefined ||
    branches.some((branch) => branch.toLowerCase() === name)
  );
};

const branchOfDirectory = remembered((directory) => {
  try {
    return execFileSync(
      "git",
      ["-C", directory, "symbolic-ref", "--quiet", "--short", "HEAD"],
      {
        encoding: "utf8",
        env: gitEnvironment(),
        stdio: ["ignore", "pipe", "ignore"],
      },
    ).trim();
  } catch {
    return undefined;
  }
});

const baseBranchFindings = (
  sub,
  options,
  statics,
  parsed,
  invocation,
  context,
) => {
  const base = context.rules.git.baseBranch;
  const isBase = (text) => baseOrUnknown(text, base.branches);
  const creating =
    base.creating
      .filter((rule) => rule.subcommands.includes(sub))
      .some((rule) => optionHits(rule, options).length > 0) &&
    statics.length > 0 &&
    isBase(statics[0].word.text)
      ? [
          finding(
            "git-history",
            `git ${sub}로 기준 브랜치를 다시 만들지 않는다`,
          ),
        ]
      : [];
  const refspecs = base.refspecSubcommands.includes(sub)
    ? statics
        .filter(({ word }) => word.text.includes(":"))
        .filter(({ word }) =>
          isBase(word.text.slice(word.text.lastIndexOf(":") + 1)),
        )
        .map(({ word }) =>
          finding(
            "git-history",
            `git ${sub}의 참조 사양으로 기준 브랜치를 덮어쓰지 않는다: ${word.text}`,
          ),
        )
    : [];
  if (sub !== "reset") {
    return [...creating, ...refspecs];
  }
  const directory = aliasDirectory(parsed, invocation, context);
  const kinds = statics.map((item) =>
    resetArgumentKind(item, statics, directory, base),
  );
  const ambiguous = kinds.includes(RESET_ARGUMENT.ambiguous)
    ? [
        finding(
          "git-history",
          "git reset의 인자가 커밋인지 경로인지 풀 수 없다. 경로는 `git reset -- <경로>`로 쓴다",
        ),
      ]
    : [];
  const committing = kinds.includes(RESET_ARGUMENT.commit);
  const moving =
    committing ||
    options.some((option) =>
      base.resetOptions.includes(option.long ?? `-${option.short}`),
    );
  const reset =
    !kinds.includes(RESET_ARGUMENT.ambiguous) &&
    ((committing && isSubagent(context.role)) ||
      (moving &&
        (directory === undefined ||
          isBase(branchOfDirectory(directory) ?? ""))))
      ? [
          finding(
            "git-history",
            isSubagent(context.role)
              ? "서브에이전트는 커밋을 받는 git reset으로 브랜치 끝을 옮기지 않는다"
              : "기준 브랜치가 체크아웃된 곳(또는 알 수 없는 곳)에서 git reset으로 브랜치 끝을 옮기지 않는다",
          ),
        ]
      : [];
  return [...creating, ...refspecs, ...ambiguous, ...reset];
};

const RESET_ARGUMENT = Object.freeze({
  stay: "stay",
  path: "path",
  commit: "commit",
  ambiguous: "ambiguous",
});

const resolvesToCommit = remembered((directory, text) => {
  try {
    execFileSync(
      "git",
      ["-C", directory, "rev-parse", "--verify", "--quiet", `${text}^{commit}`],
      { env: gitEnvironment(), stdio: ["ignore", "ignore", "ignore"] },
    );
    return true;
  } catch {
    return false;
  }
});

const existsIn = (directory, text) => {
  try {
    statSync(resolve(directory, text));
    return true;
  } catch {
    return false;
  }
};

const resetArgumentKind = (item, statics, directory, base) => {
  if (item.ended) {
    return RESET_ARGUMENT.path;
  }
  if (base.resetStay.includes(item.word.text)) {
    return RESET_ARGUMENT.stay;
  }
  if (statics.some((other) => other.ended)) {
    return RESET_ARGUMENT.commit;
  }
  if (directory === undefined) {
    return RESET_ARGUMENT.ambiguous;
  }
  const path = existsIn(directory, item.word.text);
  const commit = resolvesToCommit(directory, item.word.text);
  return path && commit
    ? RESET_ARGUMENT.ambiguous
    : path
      ? RESET_ARGUMENT.path
      : RESET_ARGUMENT.commit;
};

const realOrSelf = (path) => {
  try {
    return realpathSync.native(path);
  } catch {
    return path;
  }
};

const scriptMatches = (text, script, directory, worktrees) => {
  const plain = text.replace(/^(\.\/)+/u, "");
  if (plain === script || plain.endsWith(`/${script}`)) {
    return true;
  }
  const absolute = isAbsolute(text)
    ? resolve(text)
    : directory === undefined
      ? undefined
      : resolve(directory, text);
  if (absolute === undefined) {
    return basename(text) === basename(script);
  }
  const real = realOrSelf(absolute);
  return worktrees.some(
    (root) =>
      realOrSelf(join(root, script)) === real ||
      join(root, script) === absolute,
  );
};

const nodeScript = (words, consuming) => {
  let index = 0;
  while (index < words.length) {
    const word = words[index];
    if (word.dynamic) {
      return { index, word };
    }
    if (word.text === "--") {
      return index + 1 < words.length
        ? { index: index + 1, word: words[index + 1] }
        : undefined;
    }
    if (!isOption(word)) {
      return { index, word };
    }
    index += consuming.includes(word.text) ? 2 : 1;
  }
  return undefined;
};

export const CHECKS = [
  {
    id: "harness-write-mode",
    check: (invocation, context) => {
      if (invocation.name !== "node") {
        return [];
      }
      const found = nodeScript(
        invocation.args,
        context.rules.harnessNodeValueOptions,
      );
      if (found === undefined) {
        return [];
      }
      const rest = invocation.args.slice(found.index + 1);
      const directory = currentDirectory(invocation, context);
      return context.rules.harnessScripts
        .filter(
          (entry) =>
            found.word.dynamic ||
            scriptMatches(
              found.word.text,
              entry.script,
              directory,
              context.project.worktrees ?? [],
            ),
        )
        .filter((entry) =>
          rest.some((word) => word.dynamic || entry.modes.includes(word.text)),
        )
        .map((entry) =>
          finding(
            "harness-write-mode",
            `${entry.script}의 쓰기 모드(${entry.modes.join(", ")})는 사람이 서명 커밋으로 한다 (ADR 0018)`,
          ),
        );
    },
  },
  {
    id: "dynamic-command",
    check: (invocation) =>
      invocation.problems.map((problem) => finding("dynamic-command", problem)),
  },
  {
    id: "environment",
    check: (invocation, context) =>
      environmentFindings(
        invocation.env.map((assignment) => assignment.name),
        context.rules,
        "environment",
      ),
  },
  {
    id: "exporters",
    check: (invocation, context) => {
      if (!context.rules.environment.exporters.includes(invocation.name)) {
        return [];
      }
      const staticName = (word) =>
        !word.dynamic || /^[A-Za-z_][A-Za-z0-9_]*\+?=/u.test(word.raw);
      const found = invocation.args.filter((word) => !isOption(word));
      return [
        ...found
          .filter((word) => !staticName(word))
          .map((word) =>
            finding(
              "exporters",
              `${invocation.name}의 인자가 동적이다: ${word.raw}`,
            ),
          ),
        ...environmentFindings(
          found.filter(staticName).map((word) => word.text.split(/\+?=/u)[0]),
          context.rules,
          "exporters",
        ),
      ];
    },
  },
  {
    id: "shadowing",
    check: (invocation) => {
      if (invocation.shadows !== undefined) {
        return [
          finding(
            "shadowing",
            `함수 정의로 ${invocation.shadows}를 가리지 않는다`,
          ),
        ];
      }
      return invocation.name === "alias" &&
        invocation.args.some((word) => word.dynamic || word.text.includes("="))
        ? [finding("shadowing", "alias를 정의하지 않는다")]
        : [];
    },
  },
  {
    id: "git-global-options",
    check: (invocation, context) => {
      if (invocation.name !== "git") {
        return [];
      }
      const parsed = parseGit(invocation.args);
      const found = parsed.globals
        .filter((global) =>
          context.rules.git.deniedGlobalOptions.includes(global.option),
        )
        .filter(
          (global) =>
            !(
              global.option === "-c" &&
              global.value !== undefined &&
              !global.value.dynamic &&
              context.rules.git.allowedConfig.includes(global.value.text)
            ),
        )
        .filter(
          (global) =>
            !(global.option === "--exec-path" && global.value === undefined),
        )
        .map((global) =>
          finding(
            "git-global-options",
            `git ${global.option}로 git 설정·저장소 위치를 바꾸지 않는다`,
          ),
        );
      return parsed.dynamic
        ? [
            ...found,
            finding(
              "git-global-options",
              "git의 전역 인자나 하위 명령이 동적이다",
            ),
          ]
        : found;
    },
  },
  {
    id: "git-subcommands",
    check: (invocation, context) => {
      const parsed = gitParts(invocation);
      if (parsed === undefined) {
        return [];
      }
      const { sub, rest } = parsed;
      const git = context.rules.git;
      if (isAbort(sub, rest, context.rules)) {
        return [];
      }
      if (git.alwaysDenied.includes(sub)) {
        return [
          finding(
            "git-subcommands",
            `git ${sub}는 하네스가 막는다 (docs/harness.md 7.3)`,
          ),
        ];
      }
      if (!isSubagent(context.role)) {
        return [];
      }
      const merging =
        sub === "merge" &&
        !(
          context.role === ROLE.implementer &&
          implementerMerge(rest, context.rules)
        );
      return git.subagentDenied.includes(sub) || merging
        ? [
            finding(
              "git-subcommands",
              `서브에이전트는 git ${sub}를 하지 않는다. 푸시와 병합은 메인만 한다. 구현자는 git merge master만 쓴다 (docs/workflow.md 2장, ADR 0017)`,
            ),
          ]
        : [];
    },
  },
  {
    id: "git-options",
    check: (invocation, context) => {
      const parsed = gitParts(invocation);
      if (
        parsed === undefined ||
        isAbort(parsed.sub, parsed.rest, context.rules)
      ) {
        return [];
      }
      const { sub, rest } = parsed;
      const git = context.rules.git;
      const { options, positionals } = gitOptionWords(sub, rest, context.rules);
      const rulesFor = [
        ...git.options,
        ...(isSubagent(context.role) ? git.subagentOptions : []),
      ].filter((rule) => rule.subcommands.includes(sub));
      const found = rulesFor.flatMap((rule) =>
        optionHits(rule, options).map((option) =>
          finding(
            rule.rule,
            `git ${sub} ${option.long ?? `-${option.short}`}는 하네스가 막는다 (docs/harness.md 7.3, ADR 0017)`,
          ),
        ),
      );
      const statics = positionals.filter(({ word }) => !word.dynamic);
      const plus = git.plusRefspec.includes(sub)
        ? statics
            .filter(({ word }) => word.text.startsWith("+"))
            .map(({ word }) =>
              finding("git-force-push", `강제 refspec ${word.text}`),
            )
        : [];
      const subagent = isSubagent(context.role);
      const refspec =
        subagent && sub === "fetch" && git.fetchRefspec
          ? statics
              .filter(({ word }) => word.text.includes(":"))
              .map(({ word }) =>
                finding(
                  "git-history",
                  `서브에이전트는 fetch로 참조를 덮어쓰지 않는다: ${word.text}`,
                ),
              )
          : [];
      const forced =
        sub === "branch" &&
        options.some((option) =>
          git.forcedBranch.options.includes(option.long ?? `-${option.short}`),
        ) &&
        (statics.some(
          ({ word }) =>
            referenceName(word.text) !== undefined &&
            baseOrUnknown(word.text, git.forcedBranch.branches),
        ) ||
          (statics.length > 0 &&
            referenceName(statics[0].word.text) === undefined))
          ? [
              finding(
                "git-history",
                "기준 브랜치를 강제로 옮기거나 지우지 않는다 (ADR 0018)",
              ),
            ]
          : [];
      const dynamic = git.dynamicSensitive.includes(sub)
        ? positionals
            .filter(({ word, ended }) => word.dynamic && !ended)
            .map(({ word }) =>
              finding(
                "git-dynamic-arguments",
                `git ${sub}의 인자가 동적이다: ${word.raw}`,
              ),
            )
        : [];
      return [
        ...found,
        ...plus,
        ...refspec,
        ...forced,
        ...baseBranchFindings(
          sub,
          options,
          statics,
          parsed,
          invocation,
          context,
        ),
        ...dynamic,
      ];
    },
  },
  {
    id: "lefthook",
    check: (invocation, context) => {
      if (invocation.name !== "lefthook") {
        return [];
      }
      const sub = invocation.args.find((word) => !isOption(word));
      return sub !== undefined &&
        (sub.dynamic || context.rules.lefthook.denied.includes(sub.text))
        ? [
            finding(
              "lefthook",
              `lefthook ${sub.raw}로 git 훅 상태를 바꾸지 않는다`,
            ),
          ]
        : [];
    },
  },
  {
    id: "denied-commands",
    check: (invocation, context) => {
      const commands = context.rules.commands;
      const denied =
        commands.denied.includes(invocation.name) ||
        (isSubagent(context.role) &&
          commands.subagentDenied.includes(invocation.name));
      return denied
        ? [
            finding(
              "denied-commands",
              `${invocation.name}는 하네스가 막는다 (docs/harness.md 7.3, ADR 0017)`,
            ),
          ]
        : [];
    },
  },
  {
    id: "tool-settings",
    check: (invocation, context) => {
      const tools = context.rules.tools;
      const text = invocation.args
        .filter((word) => !isOption(word))
        .map((word) => word.text)
        .join(" ");
      const startsWith = (list) =>
        list.some((entry) => text === entry || text.startsWith(`${entry} `));
      const rustup =
        invocation.name === "rustup" && startsWith(tools.rustup.denied);
      const packages =
        tools.packageConfig.commands.includes(invocation.name) &&
        startsWith(tools.packageConfig.denied);
      const just =
        invocation.name === "just" &&
        invocation.args.some((word) =>
          tools.just.deniedOptions.some(
            (option) =>
              word.text === option || word.text.startsWith(`${option}=`),
          ),
        );
      return rustup || packages || just
        ? [
            finding(
              "tool-settings",
              `${invocation.name}의 설정·툴체인·레시피 위치를 바꾸지 않는다 (ADR 0017)`,
            ),
          ]
        : [];
    },
  },
  {
    id: "rust-flags",
    check: (invocation, context) => {
      const rust = context.rules.rust;
      if (!rust.tools.includes(invocation.name)) {
        return [];
      }
      const texts = invocation.args.map((word) => word.text);
      const separator = texts.indexOf("--");
      const before = separator === -1 ? texts : texts.slice(0, separator);
      const after = separator === -1 ? [] : texts.slice(separator + 1);
      const anywhere = texts.filter((text) =>
        rust.anywhere.some(
          (flag) => text === flag || text.startsWith(`${flag}=`),
        ),
      );
      const cargoOptions =
        invocation.name === "cargo"
          ? before.filter(
              (text) =>
                rust.cargoOptions.some(
                  (flag) => text === flag || text.startsWith(`${flag}=`),
                ) || text.startsWith("+"),
            )
          : [];
      const sub = before.find(
        (text) => !text.startsWith("-") && !text.startsWith("+"),
      );
      const lintTarget =
        invocation.name === "cargo"
          ? rust.lintFlagSubcommands.includes(sub)
            ? after
            : []
          : texts;
      const lint = lintTarget.filter((text) =>
        rust.lintFlags.some((flag) =>
          flag.startsWith("--")
            ? text === flag || text.startsWith(`${flag}=`)
            : text.startsWith(flag),
        ),
      );
      const forwarded =
        invocation.name === "cargo" && sub === "run"
          ? after
              .flatMap((text) => text.split(/\s+/u))
              .filter((token) =>
                context.rules.watched.includes(basename(token).toLowerCase()),
              )
          : [];
      return [
        ...[...anywhere, ...cargoOptions, ...lint].map((text) =>
          finding(
            "rust-flags",
            `${invocation.name} ${text}로 Rust 린트·설정을 바꾸지 않는다 (docs/harness.md 7.3)`,
          ),
        ),
        ...forwarded.map((token) =>
          finding("watched-argument", `cargo run이 ${token}를 실행할 수 있다`),
        ),
      ];
    },
  },
  {
    id: "hidden-arguments",
    check: (invocation, context) =>
      invocation.hidden &&
      context.rules.hiddenArguments.includes(invocation.name)
        ? [
            finding(
              "hidden-arguments",
              `xargs·find가 ${invocation.name}에 넘기는 인자를 볼 수 없다`,
            ),
          ]
        : [],
  },
  {
    id: "watched-argument",
    check: (invocation, context) => {
      if (
        invocation.name === undefined ||
        handledNames(context).has(invocation.name)
      ) {
        return [];
      }
      const hits = words(invocation).filter((token) =>
        context.rules.watched.includes(token),
      );
      return hits.length > 0
        ? [
            finding(
              "watched-argument",
              `${invocation.name}가 넘겨받은 ${hits[0]}를 실행할 수 있다. 하네스가 풀지 못하는 실행기는 쓰지 않는다`,
            ),
          ]
        : [];
    },
  },
  {
    id: "inline-code",
    check: (invocation, context) => {
      const rules = context.rules;
      const subagent = isSubagent(context.role);
      const family = interpreterFamily(invocation.name, rules);
      if (family !== undefined) {
        const codes = inlineCode(invocation, rules.interpreters[family]);
        if (codes.length === 0) {
          return [];
        }
        if (subagent) {
          return [subagentCode(invocation)];
        }
        return codes.flatMap((code) =>
          code.dynamic
            ? [
                finding(
                  "inline-code",
                  `${invocation.name}에 넘긴 코드가 동적이다`,
                ),
              ]
            : keywordFindings(code, invocation, context),
        );
      }
      if (rules.awk.includes(invocation.name)) {
        const programs = awkCode(invocation);
        if (
          subagent &&
          programs.some((code) => code.dynamic || AWK_WRITES.test(code.text))
        ) {
          return [subagentCode(invocation)];
        }
        return programs.flatMap((code) =>
          keywordFindings(code, invocation, context),
        );
      }
      if (invocation.name === "sed") {
        const scripts = sedScripts(invocation.args);
        const writes = scripts.some(
          (code) =>
            code.dynamic ||
            SED_COMMAND.test(code.text) ||
            SED_FLAG.test(code.text),
        );
        return subagent && writes ? [subagentCode(invocation)] : [];
      }
      const editor = rules.editors[invocation.name];
      return subagent &&
        editor !== undefined &&
        invocation.args.some((word) =>
          editor.some((option) => word.text.startsWith(option)),
        )
        ? [subagentCode(invocation)]
        : [];
    },
  },
  {
    id: "write-targets",
    check: (invocation, context) => {
      const targets = [
        ...invocation.command.redirects
          .filter((item) => !["<", "<&"].includes(item.operator))
          .map((item) => item.target),
        ...writeTargets(invocation, context.definitions.writers),
      ];
      const linked = hardLinkCreation(invocation)
        ? [
            finding(
              "hard-link",
              `${invocation.name}로 하드 링크를 만들지 않는다`,
            ),
          ]
        : [];
      return [
        ...linked,
        ...targets.flatMap((target) =>
          targetFindings(target, invocation, context),
        ),
      ];
    },
  },
  {
    id: "chmod",
    check: (invocation, context) => {
      if (!context.rules.commands.chmod.includes(invocation.name)) {
        return [];
      }
      const targets = operands(invocation.args).slice(1);
      return targets.length > 0 &&
        targets.every((target) => scopedTarget(target, invocation, context))
        ? []
        : [
            finding(
              "chmod",
              "chmod는 임시 폴더나 자기 쓰기 범위의 파일에만 쓴다",
            ),
          ];
    },
  },
];

const shellString = (invocation) => {
  const args = invocation.args;
  let index = 0;
  let command = false;
  while (
    index < args.length &&
    (isOption(args[index]) || args[index].text.startsWith("+"))
  ) {
    const text = args[index].text;
    if (/^-[a-zA-Z]*c[a-zA-Z]*$/u.test(text)) {
      command = true;
    }
    index += ["-o", "+o", "-O", "+O", "--rcfile", "--init-file"].includes(text)
      ? 2
      : 1;
  }
  if (command) {
    const script = args[index];
    if (script === undefined) {
      return { findings: [] };
    }
    return script.dynamic
      ? {
          findings: [
            finding("shell-string", `${invocation.name} -c 문자열이 동적이다`),
          ],
        }
      : { scripts: [script.text] };
  }
  if (index < args.length) {
    return { findings: [] };
  }
  const inputs = invocation.command.inputs;
  if (inputs.length === 0) {
    return invocation.command.piped
      ? {
          findings: [
            finding(
              "shell-string",
              `표준 입력으로 ${invocation.name}에 스크립트를 넘기지 않는다`,
            ),
          ],
        }
      : { findings: [] };
  }
  return inputs.some((input) => input.dynamic)
    ? {
        findings: [
          finding(
            "shell-string",
            `${invocation.name}에 넘긴 스크립트가 동적이다`,
          ),
        ],
      }
    : { scripts: inputs.map((input) => input.text) };
};

export const EXPANDERS = [
  {
    id: "shell-string",
    expand: (invocation, context) =>
      context.rules.shells.includes(invocation.name)
        ? shellString(invocation)
        : {},
  },
  {
    id: "eval",
    expand: (invocation) => {
      if (invocation.name !== "eval") {
        return {};
      }
      return invocation.args.some((word) => word.dynamic)
        ? { findings: [finding("eval", "eval의 인자가 동적이다")] }
        : {
            scripts: [invocation.args.map((word) => word.text).join(" ")],
          };
    },
  },
  {
    id: "trap",
    expand: (invocation) => {
      if (invocation.name !== "trap") {
        return {};
      }
      const action = invocation.args.find((word) => !isOption(word));
      if (action === undefined) {
        return {};
      }
      return action.dynamic
        ? { findings: [finding("trap", "trap의 동작이 동적이다")] }
        : { scripts: [action.text] };
    },
  },
  {
    id: "find-exec",
    expand: (invocation) => {
      if (invocation.name !== "find") {
        return {};
      }
      const args = invocation.args;
      const nested = [];
      args.forEach((word, index) => {
        if (["-exec", "-execdir", "-ok", "-okdir"].includes(word.text)) {
          const end = args.findIndex(
            (candidate, position) =>
              position > index && [";", "+"].includes(candidate.text),
          );
          const found = args
            .slice(index + 1, end === -1 ? args.length : end)
            .map((item) =>
              item.text.includes("{}") ? { ...item, dynamic: true } : item,
            );
          nested.push({
            ...invocation.command,
            assignments: [],
            words: found,
            redirects: [],
            inputs: [],
          });
        }
      });
      return { commands: nested, hidden: true };
    },
  },
  {
    id: "git-alias",
    expand: (invocation, context) => {
      const parsed = gitParts(invocation);
      if (
        parsed === undefined ||
        context.rules.git.builtins.includes(parsed.sub)
      ) {
        return {};
      }
      const directory = aliasDirectory(parsed, invocation, context);
      if (directory === undefined) {
        return {
          findings: [
            finding(
              "git-alias",
              `git ${parsed.sub}가 별칭인지 풀 수 없다(-C 경로나 현재 폴더를 모른다)`,
            ),
          ],
        };
      }
      const alias = lookupAlias(parsed.sub, directory);
      if (alias === undefined || alias === "") {
        return {};
      }
      const rest = parsed.rest.map(quoteWord).join(" ");
      if (alias.startsWith("!")) {
        return { scripts: [`${alias.slice(1)} ${rest}`] };
      }
      const before = invocation.args
        .slice(0, parsed.index)
        .map(quoteWord)
        .join(" ");
      return { scripts: [`git ${before} ${alias} ${rest}`] };
    },
  },
];

const isDirectory = (path) => {
  try {
    return statSync(path).isDirectory();
  } catch {
    return false;
  }
};

const changeDirectory = (invocation, context, grouped) => {
  if (invocation.name === "popd") {
    return undefined;
  }
  const target = operands(invocation.args)[0];
  if (target === undefined) {
    return homedir();
  }
  if (target.dynamic || target.glob || target.text === "-" || grouped) {
    return undefined;
  }
  const path = absoluteTarget(
    target.text,
    currentDirectory(invocation, context),
  );
  return path !== undefined && isDirectory(path) ? path : undefined;
};

const nestedContext = (context) => ({ ...context });

const analyzeNested = (script, context, depth) => {
  const child = nestedContext(context);
  const found = analyzeScript(script, child, depth);
  if (child.cwd !== context.cwd) {
    context.cwd = undefined;
  }
  return found;
};

const scriptFindings = (invocation, context, depth) => {
  const parsed = parseScript(invocation.script);
  if (parsed.commands.length !== 1 || parsed.problems.length > 0) {
    return [finding("env-split", "env -S 문자열이 명령 하나로 풀리지 않는다")];
  }
  return analyzeNested(invocation.script, context, depth + 1);
};

const analyzeCommand = (command, context, depth, hidden, grouped) => {
  const invocation = unwrap(command, context.definitions, context.rules);
  invocation.hidden ||= hidden;
  const always = [
    "dynamic-command",
    "environment",
    "write-targets",
    "shadowing",
  ];
  const findings = context.definitions.checks.flatMap((item) =>
    invocation.executes || always.includes(item.id)
      ? item.check(invocation, context)
      : [],
  );
  const scope = invocation.cwdUnknown
    ? { ...context, cwd: undefined }
    : context;
  if (invocation.script !== undefined) {
    findings.push(...scriptFindings(invocation, scope, depth));
  }
  if (invocation.executes && invocation.name !== undefined) {
    for (const expander of context.definitions.expanders) {
      const result = expander.expand(invocation, scope);
      findings.push(...(result.findings ?? []));
      for (const script of result.scripts ?? []) {
        findings.push(...analyzeNested(script, scope, depth + 1));
      }
      for (const nested of result.commands ?? []) {
        findings.push(
          ...analyzeCommand(
            nested,
            nestedContext(scope),
            depth + 1,
            result.hidden === true,
            true,
          ),
        );
      }
    }
  }
  if (["cd", "pushd", "popd"].includes(invocation.name)) {
    context.cwd = changeDirectory(invocation, context, grouped);
  }
  return findings;
};

export const analyzeScript = (source, context, depth = 0) => {
  if (depth > MAX_DEPTH) {
    return [finding("unparsable", "명령이 너무 깊게 겹쳐 있다")];
  }
  const parsed = parseScript(source);
  const findings = parsed.problems.map((problem) =>
    finding("unparsable", `명령을 해석하지 못했다: ${problem}`),
  );
  parsed.commands.forEach((command) => {
    findings.push(
      ...analyzeCommand(command, context, depth, false, parsed.grouped),
    );
  });
  for (const substitution of parsed.substitutions) {
    findings.push(...analyzeNested(substitution, context, depth + 1));
  }
  return findings;
};

export const DEFINITIONS = Object.freeze({
  checks: CHECKS,
  expanders: EXPANDERS,
  wrappers: WRAPPERS,
  writers: WRITERS,
  reserved: RESERVED_PREFIXES,
});

export const bashFindings = (input, project, definitions = DEFINITIONS) => {
  const toolInput = input.tool_input ?? {};
  const context = {
    role: roleOf(input),
    project,
    rules: project.rules,
    cwd:
      typeof input.cwd === "string" && input.cwd !== ""
        ? input.cwd
        : process.cwd(),
    definitions,
  };
  const findings = [];
  if (toolInput.dangerouslyDisableSandbox === true) {
    findings.push(
      finding(
        "sandbox-escape",
        "dangerouslyDisableSandbox로 샌드박스 밖에서 실행하지 않는다",
      ),
    );
  }
  const command = toolInput.command;
  if (typeof command !== "string") {
    return [...findings, finding("unparsable", "명령 문자열이 없다")];
  }
  return [...findings, ...analyzeScript(command, context)];
};

export const decideBash = (input, project, definitions = DEFINITIONS) =>
  decide(bashFindings(input, project, definitions), project.lock);
