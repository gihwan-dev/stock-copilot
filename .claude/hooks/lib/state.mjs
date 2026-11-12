import {
  mkdirSync,
  readFileSync,
  readdirSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { homedir } from "node:os";
import { isAbsolute, join } from "node:path";

export const STATE_RELATIVE = ".local/state/stock-copilot-harness";

export const STATE_VARIABLE = "STOCK_COPILOT_HARNESS_STATE";

export const stateDirectory = (environment = process.env, home = homedir()) => {
  const chosen = environment[STATE_VARIABLE];
  return typeof chosen === "string" && isAbsolute(chosen)
    ? chosen
    : join(home, STATE_RELATIVE);
};

export const STATUS = Object.freeze({
  passed: "passed",
  failing: "failing",
  blocked: "blocked",
  unchecked: "unchecked",
});

export const KIND = Object.freeze({
  implementer: "implementer",
  reviewer: "reviewer",
  pending: "pending",
});

const SAFE_ID = /^[A-Za-z0-9_-]{1,128}$/u;

export const isSafeId = (id) => typeof id === "string" && SAFE_ID.test(id);

const recordFile = (directory, kind, id) =>
  join(directory, "records", `${kind}-${id}.json`);

export const readRecord = (directory, kind, id) => {
  if (!isSafeId(id)) {
    return undefined;
  }
  try {
    return JSON.parse(readFileSync(recordFile(directory, kind, id), "utf8"));
  } catch {
    return undefined;
  }
};

export const writeRecord = (directory, kind, id, record) => {
  if (!isSafeId(id)) {
    throw new Error(`기록 ID가 잘못됐다: ${String(id)}`);
  }
  const folder = join(directory, "records");
  mkdirSync(folder, { recursive: true, mode: 0o700 });
  const target = recordFile(directory, kind, id);
  const temporary = `${target}.${process.pid}.tmp`;
  writeFileSync(temporary, `${JSON.stringify(record, undefined, 2)}\n`, {
    mode: 0o600,
  });
  renameSync(temporary, target);
};

const namesIn = (folder) => {
  try {
    return readdirSync(folder);
  } catch {
    return [];
  }
};

export const listRecords = (directory, kind) =>
  namesIn(join(directory, "records"))
    .filter((name) => name.startsWith(`${kind}-`) && name.endsWith(".json"))
    .map((name) => name.slice(kind.length + 1, -".json".length))
    .map((id) => readRecord(directory, kind, id))
    .filter((record) => record !== undefined);

export const claimRecord = (directory, kind, id, claimant) => {
  if (!isSafeId(id) || !isSafeId(claimant)) {
    return undefined;
  }
  const source = recordFile(directory, kind, id);
  const claimed = `${source}.claimed-${claimant}`;
  try {
    renameSync(source, claimed);
  } catch {
    return undefined;
  }
  try {
    return JSON.parse(readFileSync(claimed, "utf8"));
  } catch {
    return undefined;
  } finally {
    rmSync(claimed, { force: true });
  }
};
