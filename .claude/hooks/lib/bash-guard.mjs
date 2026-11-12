import { execFileSync } from "node:child_process";
import { existsSync, realpathSync } from "node:fs";
import { homedir } from "node:os";
import { basename, dirname, isAbsolute, join, resolve } from "node:path";
import { fixedFindings, locate, protectedFinding } from "./paths.mjs";
import { gitEnvironment } from "./project.mjs";
import { isSubagent, roleOf } from "./roles.mjs";
import { parseScript } from "./shell.mjs";
import { decide } from "./verdict.mjs";

const MAX_DEPTH = 8;

const finding = (rule, message, gated = false) => ({ rule, gated, message });

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

const splitString = (args, index) => {
  const text = args[index].text;
  const attached = text.startsWith("--split-string=");
  const value = attached
    ? literal(text.slice("--split-string=".length))
    : args[index + 1];
  if (value === undefined || value.dynamic) {
    return { problem: "env -S 문자열이 동적이다" };
  }
  const split = parseScript(value.text).commands[0] ?? {
    assignments: [],
    words: [],
  };
  return { split, rest: args.slice(index + (attached ? 1 : 2)) };
};

const unwrapEnv = (args, invocation) => {
  let index = 0;
  while (index < args.length) {
    const word = args[index];
    if (word.dynamic) {
      return { problem: "env 인자가 동적이다" };
    }
    const text = word.text;
    if (text === "-S" || text.startsWith("--split-string")) {
      const result = splitString(args, index);
      if (result.problem !== undefined) {
        return result;
      }
      invocation.env.push(...result.split.assignments);
      return { args: [...result.split.words, ...result.rest] };
    }
    const assignment = assignmentOf(word);
    if (["-u", "-C", "-P", "--unset", "--chdir"].includes(text)) {
      index += 2;
    } else if (text.startsWith("-")) {
      index += 1;
    } else if (assignment !== undefined) {
      invocation.env.push(assignment);
      index += 1;
    } else {
      break;
    }
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
  timeout: unwrapTimeout,
  gtimeout: unwrapTimeout,
  arch: unwrapArch,
  env: unwrapEnv,
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

export const unwrap = (command, definitions) => {
  const invocation = {
    name: undefined,
    args: [],
    env: [...command.assignments],
    hidden: false,
    executes: true,
    command,
    problems: [],
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
      words = words.slice(2);
      continue;
    }
    const wrapper = Object.hasOwn(definitions.wrappers, name)
      ? definitions.wrappers[name]
      : undefined;
    if (wrapper === undefined) {
      invocation.name = name;
      invocation.args = words.slice(1);
      return invocation;
    }
    const unwrapped = wrapper(words.slice(1), invocation);
    if (unwrapped.problem !== undefined) {
      invocation.problems.push(unwrapped.problem);
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

const environmentFindings = (names, rules) =>
  names
    .filter(
      (name) =>
        rules.environment.names.includes(name) ||
        rules.environment.prefixes.some((prefix) => name.startsWith(prefix)),
    )
    .map((name) =>
      finding(
        "environment",
        `${name} 환경 변수로 훅·git·Rust 검사를 바꾸지 않는다 (docs/harness.md 7.3)`,
      ),
    );

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
    const takesValue = [
      "-C",
      "-c",
      "--git-dir",
      "--work-tree",
      "--namespace",
      "--config-env",
      "--super-prefix",
      "--attr-source",
    ].includes(option);
    if (takesValue && attached === undefined) {
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
      options.push({ long: word.text });
      continue;
    }
    const cluster = word.text.slice(1);
    for (let position = 0; position < cluster.length; position += 1) {
      const character = cluster[position];
      options.push({ short: character });
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

const aliases = new Map();

const gitAlias = (sub, cwd) => {
  const key = `${cwd}\0${sub}`;
  if (!aliases.has(key)) {
    aliases.set(key, lookupAlias(sub, cwd));
  }
  return aliases.get(key);
};

const lookupAlias = (sub, cwd) => {
  try {
    return execFileSync("git", ["-C", cwd, "config", "--get", `alias.${sub}`], {
      encoding: "utf8",
      env: gitEnvironment(),
      stdio: ["ignore", "pipe", "ignore"],
    }).trim();
  } catch {
    return undefined;
  }
};

const quoteWord = (word) => `'${word.text.replaceAll("'", "'\\''")}'`;

export const CHECKS = [
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
      ),
  },
  {
    id: "exporters",
    check: (invocation, context) => {
      if (!context.rules.environment.exporters.includes(invocation.name)) {
        return [];
      }
      const operands = invocation.args.filter((word) => !isOption(word));
      const staticName = (word) =>
        !word.dynamic || /^[A-Za-z_][A-Za-z0-9_]*\+?=/u.test(word.raw);
      const dynamic = operands.filter((word) => !staticName(word));
      const names = operands
        .filter(staticName)
        .map((word) => word.text.split(/\+?=/u)[0]);
      return [
        ...dynamic.map((word) =>
          finding(
            "exporters",
            `${invocation.name}의 인자가 동적이다: ${word.raw}`,
          ),
        ),
        ...environmentFindings(names, context.rules).map((item) => ({
          ...item,
          rule: "exporters",
        })),
      ];
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
      if (invocation.name !== "git") {
        return [];
      }
      const { sub } = parseGit(invocation.args);
      if (sub === undefined) {
        return [];
      }
      if (context.rules.git.alwaysDenied.includes(sub)) {
        return [
          finding(
            "git-subcommands",
            `git ${sub}는 하네스가 막는다 (docs/harness.md 7.3)`,
          ),
        ];
      }
      if (
        isSubagent(context.role) &&
        context.rules.git.subagentDenied.includes(sub)
      ) {
        return [
          finding(
            "git-subcommands",
            `서브에이전트는 git ${sub}를 하지 않는다. 푸시와 병합은 메인만 한다 (docs/workflow.md 2장)`,
          ),
        ];
      }
      return [];
    },
  },
  {
    id: "git-options",
    check: (invocation, context) => {
      if (invocation.name !== "git") {
        return [];
      }
      const { sub, rest } = parseGit(invocation.args);
      if (sub === undefined) {
        return [];
      }
      const { options, positionals } = gitOptionWords(sub, rest, context.rules);
      const found = context.rules.git.options
        .filter((rule) => rule.subcommands.includes(sub))
        .flatMap((rule) =>
          options
            .filter(
              (option) =>
                (option.long !== undefined &&
                  rule.long.some((forbidden) =>
                    longMatches(option.long, forbidden),
                  )) ||
                (option.short !== undefined &&
                  rule.short.includes(option.short)),
            )
            .map((option) =>
              finding(
                rule.rule,
                `git ${sub} ${option.long ?? `-${option.short}`}는 하네스가 막는다 (docs/harness.md 7.3)`,
              ),
            ),
        );
      const plus = context.rules.git.plusRefspec.includes(sub)
        ? positionals
            .filter(({ word }) => !word.dynamic && word.text.startsWith("+"))
            .map(({ word }) =>
              finding("git-force-push", `강제 refspec ${word.text}`),
            )
        : [];
      const dynamic = context.rules.git.dynamicSensitive.includes(sub)
        ? positionals
            .filter(({ word, ended }) => word.dynamic && !ended)
            .map(({ word }) =>
              finding(
                "git-dynamic-arguments",
                `git ${sub}의 인자가 동적이다: ${word.raw}`,
              ),
            )
        : [];
      return [...found, ...plus, ...dynamic];
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
    check: (invocation, context) =>
      context.rules.commands.denied.includes(invocation.name)
        ? [
            finding(
              "denied-commands",
              `${invocation.name}는 하네스가 막는다 (docs/harness.md 7.3)`,
            ),
          ]
        : [],
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
      return [...anywhere, ...cargoOptions, ...lint].map((text) =>
        finding(
          "rust-flags",
          `${invocation.name} ${text}로 Rust 린트·설정을 바꾸지 않는다 (docs/harness.md 7.3)`,
        ),
      );
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
    id: "inline-code",
    check: (invocation, context) => {
      const family = interpreterFamily(invocation.name, context.rules);
      if (family === undefined) {
        return [];
      }
      const codes = inlineCode(invocation, context.rules.interpreters[family]);
      return codes.flatMap((code) => {
        if (code.dynamic) {
          return [
            finding("inline-code", `${invocation.name}에 넘긴 코드가 동적이다`),
          ];
        }
        const lowered = code.text.toLowerCase();
        const keywords = [
          ...context.rules.inlineCodeKeywords.filter((keyword) =>
            lowered.includes(keyword.toLowerCase()),
          ),
          ...[
            ...context.rules.environment.names,
            ...context.rules.environment.prefixes,
          ].filter((name) => code.text.includes(name)),
        ];
        return keywords.map((keyword) =>
          finding(
            "inline-code",
            `${invocation.name} 코드에 하네스가 막는 낱말 ${keyword}가 있다`,
          ),
        );
      });
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
      return targets.flatMap((target) => targetFindings(target, context));
    },
  },
];

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
  }
  return codes;
};

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
  if (isAbsolute(text)) {
    return resolve(text);
  }
  return cwd === undefined ? undefined : resolve(cwd, text);
};

const realTarget = (path) => {
  let existing = path;
  const rest = [];
  while (!existsSync(existing) && dirname(existing) !== existing) {
    rest.unshift(basename(existing));
    existing = dirname(existing);
  }
  try {
    return join(realpathSync.native(existing), ...rest);
  } catch {
    return path;
  }
};

const targetFindings = (target, context) => {
  if (target.dynamic || target.glob) {
    return [];
  }
  const lexical = absoluteTarget(target.text, context.cwd);
  if (lexical === undefined) {
    return [];
  }
  const paths = [...new Set([lexical, realTarget(lexical)])];
  return paths.flatMap((path) => {
    const fixed = fixedFindings(path.slice(1), context.rules).filter(
      (item) => item.rule === "forbidden-path",
    );
    if (fixed.length > 0) {
      return fixed;
    }
    const located = locate(path, context.project.worktrees);
    if (located.relative === undefined) {
      return [];
    }
    const nested = fixedFindings(located.relative, context.rules);
    return nested.length > 0
      ? nested
      : protectedFinding(located.relative, context.project.allowlist);
  });
};

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
        : { scripts: [invocation.args.map((word) => word.text).join(" ")] };
    },
  },
  {
    id: "alias",
    expand: (invocation) => {
      if (invocation.name !== "alias") {
        return {};
      }
      const values = invocation.args.filter((word) => word.text.includes("="));
      return values.some((word) => word.dynamic)
        ? { findings: [finding("alias", "alias 정의가 동적이다")] }
        : {
            scripts: values.map((word) =>
              word.text.slice(word.text.indexOf("=") + 1),
            ),
          };
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
          const words = args
            .slice(index + 1, end === -1 ? args.length : end)
            .map((item) =>
              item.text.includes("{}") ? { ...item, dynamic: true } : item,
            );
          nested.push({
            ...invocation.command,
            assignments: [],
            words,
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
      if (invocation.name !== "git") {
        return {};
      }
      const parsed = parseGit(invocation.args);
      if (
        parsed.sub === undefined ||
        context.rules.git.alwaysDenied.includes(parsed.sub) ||
        context.rules.git.subagentDenied.includes(parsed.sub) ||
        context.rules.git.options.some((rule) =>
          rule.subcommands.includes(parsed.sub),
        )
      ) {
        return {};
      }
      const alias = gitAlias(parsed.sub, context.cwd);
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

const updateDirectory = (invocation, context) => {
  if (!["cd", "pushd"].includes(invocation.name)) {
    return;
  }
  const target = operands(invocation.args)[0];
  if (target === undefined) {
    context.cwd = homedir();
  } else {
    context.cwd = target.dynamic
      ? undefined
      : absoluteTarget(target.text, context.cwd);
  }
};

const analyzeCommand = (command, context, depth, hidden) => {
  const invocation = unwrap(command, context.definitions);
  invocation.hidden ||= hidden;
  const findings = context.definitions.checks.flatMap((item) =>
    invocation.executes ||
    ["dynamic-command", "environment", "write-targets"].includes(item.id)
      ? item.check(invocation, context)
      : [],
  );
  if (invocation.executes && invocation.name !== undefined) {
    for (const expander of context.definitions.expanders) {
      const result = expander.expand(invocation, context);
      findings.push(...(result.findings ?? []));
      for (const script of result.scripts ?? []) {
        findings.push(...analyzeScript(script, context, depth + 1));
      }
      for (const nested of result.commands ?? []) {
        findings.push(
          ...analyzeCommand(nested, context, depth + 1, result.hidden === true),
        );
      }
    }
  }
  updateDirectory(invocation, context);
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
    findings.push(...analyzeCommand(command, context, depth, false));
  });
  for (const substitution of parsed.substitutions) {
    findings.push(...analyzeScript(substitution, context, depth + 1));
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
