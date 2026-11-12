const BLANKS = new Set([" ", "\t"]);
const META = new Set([" ", "\t", "\n", ";", "&", "|", "(", ")", "<", ">"]);
const OPERATORS = [
  "&&",
  "||",
  ";;&",
  ";;",
  ";&",
  "|&",
  "|",
  ";",
  "&",
  "(",
  ")",
];
const REDIRECTS = [
  "<<<",
  "<<-",
  "<<",
  "<>",
  "<&",
  "<",
  "&>>",
  "&>",
  ">>",
  ">|",
  ">&",
  ">",
];
const NAME_START = /[A-Za-z_]/u;
const NAME_PART = /[A-Za-z0-9_]/u;
const SPECIAL_PARAMETERS = new Set(["@", "*", "#", "?", "$", "!", "-"]);
const ASSIGNMENT = /^([A-Za-z_][A-Za-z0-9_]*)(\[[^\]]*\])?\+?=/u;
const ANSI_ESCAPES = new Map([
  ["n", "\n"],
  ["t", "\t"],
  ["r", "\r"],
  ["a", "\u0007"],
  ["b", "\b"],
  ["e", "\u001b"],
  ["E", "\u001b"],
  ["f", "\f"],
  ["v", "\v"],
  ["\\", "\\"],
  ["'", "'"],
  ['"', '"'],
  ["?", "?"],
]);

const skipSingle = (source, index) => {
  const end = source.indexOf("'", index + 1);
  return end === -1 ? source.length : end + 1;
};

const skipDouble = (source, index) => {
  let position = index + 1;
  while (position < source.length && source[position] !== '"') {
    position += source[position] === "\\" ? 2 : 1;
  }
  return position + 1;
};

const skipBacktick = (source, index) => {
  let position = index + 1;
  while (position < source.length && source[position] !== "`") {
    position += source[position] === "\\" ? 2 : 1;
  }
  return position + 1;
};

const closing = (source, start, open, close) => {
  let depth = 1;
  let position = start;
  while (position < source.length) {
    const character = source[position];
    if (character === "\\") {
      position += 2;
    } else if (character === "'") {
      position = skipSingle(source, position);
    } else if (character === '"') {
      position = skipDouble(source, position);
    } else if (character === "`") {
      position = skipBacktick(source, position);
    } else {
      depth += character === open ? 1 : character === close ? -1 : 0;
      if (depth === 0) {
        return position;
      }
      position += 1;
    }
  }
  return -1;
};

const decodeAnsi = (body) => {
  let text = "";
  let position = 0;
  while (position < body.length) {
    const character = body[position];
    if (character !== "\\") {
      text += character;
      position += 1;
      continue;
    }
    const next = body[position + 1] ?? "";
    const known = ANSI_ESCAPES.get(next);
    const hex = /^x([0-9A-Fa-f]{1,2})/u.exec(body.slice(position + 1));
    const unicode = /^[uU]([0-9A-Fa-f]{1,8})/u.exec(body.slice(position + 1));
    const octal = /^0?([0-7]{1,3})/u.exec(body.slice(position + 1));
    if (known !== undefined) {
      text += known;
      position += 2;
    } else if (hex !== null) {
      text += String.fromCodePoint(Number.parseInt(hex[1], 16));
      position += 1 + hex[0].length;
    } else if (unicode !== null) {
      text += String.fromCodePoint(Number.parseInt(unicode[1], 16));
      position += 1 + unicode[0].length;
    } else if (octal !== null) {
      text += String.fromCodePoint(Number.parseInt(octal[1], 8));
      position += 1 + octal[0].length;
    } else {
      text += `\\${next}`;
      position += 2;
    }
  }
  return text;
};

const ansiEnd = (source, start) => {
  let position = start;
  while (position < source.length && source[position] !== "'") {
    position += source[position] === "\\" ? 2 : 1;
  }
  return position;
};

const unescapeBacktick = (body) => body.replace(/\\([\\`$])/gu, "$1");

const createState = (source) => ({
  source,
  index: 0,
  commands: [],
  problems: [],
  substitutions: [],
  pending: [],
  current: undefined,
});

const freshCommand = (piped = false) => ({
  assignments: [],
  words: [],
  redirects: [],
  inputs: [],
  piped,
});

const dollar = (state, word, quoted) => {
  const { source } = state;
  const start = state.index;
  const next = source[start + 1];
  if (!quoted && next === "'") {
    const end = ansiEnd(source, start + 2);
    if (end >= source.length) {
      state.problems.push("닫히지 않은 $'…'");
    }
    word.text += decodeAnsi(source.slice(start + 2, end));
    word.quoted = true;
    state.index = end + 1;
    return;
  }
  if (!quoted && next === '"') {
    state.index = start + 1;
    return;
  }
  if (next === "(" || next === "{") {
    const arithmetic = next === "(" && source[start + 2] === "(";
    const open = next;
    const close = next === "(" ? ")" : "}";
    const end = closing(source, start + 2, open, close);
    if (end === -1) {
      state.problems.push(`닫히지 않은 $${open}`);
      state.index = source.length;
      word.dynamic = true;
      return;
    }
    const body = source.slice(start + 2, end);
    if (next === "(" && !arithmetic) {
      state.substitutions.push(body);
    } else {
      state.substitutions.push(...substitutionsIn(body));
    }
    word.text += source.slice(start, end + 1);
    word.dynamic = true;
    state.index = end + 1;
    return;
  }
  if (next !== undefined && (NAME_START.test(next) || /[0-9]/u.test(next))) {
    let position = start + 1;
    const part = NAME_START.test(next) ? NAME_PART : /[0-9]/u;
    while (position < source.length && part.test(source[position])) {
      position += 1;
    }
    word.text += source.slice(start, position);
    word.dynamic = true;
    state.index = position;
    return;
  }
  if (next !== undefined && SPECIAL_PARAMETERS.has(next)) {
    word.text += source.slice(start, start + 2);
    word.dynamic = true;
    state.index = start + 2;
    return;
  }
  word.text += "$";
  state.index = start + 1;
};

const backtick = (state, word) => {
  const { source } = state;
  const end = skipBacktick(source, state.index) - 1;
  if (end >= source.length) {
    state.problems.push("닫히지 않은 `");
  }
  state.substitutions.push(
    unescapeBacktick(source.slice(state.index + 1, end)),
  );
  word.text += source.slice(state.index, end + 1);
  word.dynamic = true;
  state.index = end + 1;
};

const doubleQuoted = (state, word) => {
  const { source } = state;
  state.index += 1;
  word.quoted = true;
  while (state.index < source.length && source[state.index] !== '"') {
    const character = source[state.index];
    if (character === "\\") {
      const next = source[state.index + 1] ?? "";
      if (next === "\n") {
        state.index += 2;
      } else if (["$", "`", '"', "\\"].includes(next)) {
        word.text += next;
        state.index += 2;
      } else {
        word.text += "\\";
        state.index += 1;
      }
    } else if (character === "$") {
      dollar(state, word, true);
    } else if (character === "`") {
      backtick(state, word);
    } else {
      word.text += character;
      state.index += 1;
    }
  }
  if (state.index >= source.length) {
    state.problems.push('닫히지 않은 "');
  }
  state.index += 1;
};

const processSubstitution = (state, word) => {
  const { source } = state;
  const end = closing(source, state.index + 2, "(", ")");
  if (end === -1) {
    state.problems.push("닫히지 않은 <( 또는 >(");
    state.index = source.length;
  } else {
    state.substitutions.push(source.slice(state.index + 2, end));
    word.text += source.slice(state.index, end + 1);
    state.index = end + 1;
  }
  word.dynamic = true;
};

const readWord = (state) => {
  const { source } = state;
  const start = state.index;
  const word = {
    text: "",
    dynamic: false,
    quoted: false,
    glob: false,
    raw: "",
  };
  while (state.index < source.length) {
    const character = source[state.index];
    const next = source[state.index + 1];
    if ((character === "<" || character === ">") && next === "(") {
      processSubstitution(state, word);
    } else if (META.has(character)) {
      break;
    } else if (character === "\\") {
      if (next === "\n") {
        state.index += 2;
      } else {
        word.text += next ?? "";
        word.quoted = true;
        state.index += 2;
      }
    } else if (character === "'") {
      const end = skipSingle(source, state.index);
      if (source[end - 1] !== "'" || end - 1 === state.index) {
        state.problems.push("닫히지 않은 '");
      }
      word.text += source.slice(state.index + 1, end - 1);
      word.quoted = true;
      state.index = end;
    } else if (character === '"') {
      doubleQuoted(state, word);
    } else if (character === "$") {
      dollar(state, word, false);
    } else if (character === "`") {
      backtick(state, word);
    } else {
      if ("*?[".includes(character)) {
        word.glob = true;
      }
      if (
        character === "{" &&
        /^\{[^}]*,[^}]*\}/u.test(source.slice(state.index))
      ) {
        word.dynamic = true;
      }
      word.text += character;
      state.index += 1;
    }
  }
  word.raw = source.slice(start, state.index);
  return word;
};

export const wordOf = (text) => {
  const state = createState(text);
  return readWord(state);
};

export const substitutionsIn = (text) => {
  const state = createState(text);
  const scratch = { text: "", dynamic: false, quoted: false, glob: false };
  while (state.index < text.length) {
    const character = text[state.index];
    if (character === "\\") {
      state.index += 2;
    } else if (character === "$") {
      dollar(state, scratch, true);
    } else if (character === "`") {
      backtick(state, scratch);
    } else {
      state.index += 1;
    }
  }
  return state.substitutions;
};

const finish = (state, piped = false) => {
  const command = state.current;
  if (
    command.words.length +
      command.assignments.length +
      command.redirects.length +
      command.inputs.length >
    0
  ) {
    state.commands.push(command);
  }
  state.current = freshCommand(piped);
};

const addWord = (state, word) => {
  const command = state.current;
  const assignment = ASSIGNMENT.exec(word.raw);
  if (command.words.length === 0 && assignment !== null) {
    command.assignments.push({
      name: assignment[1],
      value: wordOf(word.raw.slice(assignment[0].length)),
    });
    return;
  }
  command.words.push(word);
};

const skipBlanks = (state) => {
  while (BLANKS.has(state.source[state.index])) {
    state.index += 1;
  }
};

const redirect = (state, operator, descriptor) => {
  state.index += operator.length;
  skipBlanks(state);
  const target = readWord(state);
  if (target.raw === "") {
    state.problems.push(`${operator} 뒤에 대상이 없다`);
    return;
  }
  const command = state.current;
  if (operator === "<<" || operator === "<<-") {
    state.pending.push({
      delimiter: target.text,
      expand: !target.quoted,
      strip: operator === "<<-",
      command,
    });
    return;
  }
  if (operator === "<<<") {
    command.inputs.push({
      kind: "herestring",
      text: target.text,
      dynamic: target.dynamic,
    });
    return;
  }
  if (
    (operator === ">&" || operator === "<&") &&
    /^(\d+-?|-)$/u.test(target.text)
  ) {
    return;
  }
  command.redirects.push({ operator, descriptor, target });
};

const heredocs = (state) => {
  const { source } = state;
  for (const pending of state.pending) {
    const lines = [];
    let closed = false;
    while (state.index < source.length) {
      const end = source.indexOf("\n", state.index);
      const stop = end === -1 ? source.length : end;
      const line = source.slice(state.index, stop);
      state.index = stop + 1;
      const compared = pending.strip ? line.replace(/^\t+/u, "") : line;
      if (compared === pending.delimiter) {
        closed = true;
        break;
      }
      lines.push(compared);
    }
    if (!closed) {
      state.index = source.length;
    }
    const text = lines.join("\n");
    if (pending.expand) {
      state.substitutions.push(...substitutionsIn(text));
    }
    pending.command.inputs.push({
      kind: "heredoc",
      text,
      dynamic: false,
      expand: pending.expand,
    });
  }
  state.pending = [];
};

const step = (state) => {
  const { source } = state;
  const character = source[state.index];
  const next = source[state.index + 1];
  if (BLANKS.has(character)) {
    state.index += 1;
  } else if (character === "\\" && next === "\n") {
    state.index += 2;
  } else if (character === "\n") {
    finish(state);
    state.index += 1;
    heredocs(state);
  } else if (character === "#") {
    const end = source.indexOf("\n", state.index);
    state.index = end === -1 ? source.length : end;
  } else if ((character === "<" || character === ">") && next === "(") {
    addWord(state, readWord(state));
  } else {
    const operator = REDIRECTS.find((candidate) =>
      source.startsWith(candidate, state.index),
    );
    if (operator !== undefined) {
      redirect(state, operator, undefined);
      return;
    }
    const separator = OPERATORS.find((candidate) =>
      source.startsWith(candidate, state.index),
    );
    if (separator !== undefined) {
      finish(state, separator === "|" || separator === "|&");
      state.index += separator.length;
      return;
    }
    const word = readWord(state);
    const following = source[state.index];
    const descriptor =
      /^\d+$/u.test(word.raw) && (following === "<" || following === ">");
    const attached = descriptor
      ? REDIRECTS.find((candidate) => source.startsWith(candidate, state.index))
      : undefined;
    if (attached !== undefined && source[state.index + 1] !== "(") {
      redirect(state, attached, word.raw);
    } else {
      addWord(state, word);
    }
  }
};

export const parseScript = (source) => {
  const state = createState(source);
  state.current = freshCommand();
  while (state.index < source.length) {
    step(state);
  }
  finish(state);
  heredocs(state);
  return {
    commands: state.commands,
    problems: state.problems,
    substitutions: state.substitutions,
  };
};
