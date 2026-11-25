import {
  parseChartDecimal,
  parseEntityId,
  parseFreeText,
  parseMoney,
  parsePercentDisplay,
  parseQuantity,
  parseTicker,
  parseTimestamp,
} from "../../apps/desktop/src/shared/lib/index.ts";
import { pendingScreenSources } from "../../apps/desktop/src/shared/ipc/index.ts";
import { settleWithSchema } from "../../apps/desktop/src/shared/ipc/schema.ts";
import * as setup from "../../apps/desktop/src/shared/ipc/setup-schema.ts";
import * as ai from "../../apps/desktop/src/shared/ipc/ai-schema.ts";
import * as news from "../../apps/desktop/src/shared/ipc/news-schema.ts";
import { responses } from "../../apps/desktop/src/shared/ipc/testing/responses.ts";
import { settingsData } from "./settings-data.mjs";

const must = (result) => {
  if (result.kind !== "ok") throw new Error("fixture value rejected");
  return result.value;
};

const decode = (value, schema) =>
  Promise.resolve(settleWithSchema({ status: "ok", data: value }, schema));

export const fx = {
  id: (value) => must(parseEntityId(value)),
  free: (value) => must(parseFreeText(value)),
  krw: (value) => must(parseMoney(value, "KRW")),
  usd: (value) => must(parseMoney(value, "USD")),
  pct: (value) => must(parsePercentDisplay(value)),
  qty: (value) => must(parseQuantity(value)),
  ticker: (value) => must(parseTicker(value)),
  at: (value) => must(parseTimestamp(value)),
  dec: (value) => must(parseChartDecimal(value)),
  ok: (value) => Promise.resolve({ kind: "ok", value }),
  fail: (kind) =>
    Promise.resolve({
      kind: "err",
      error: { kind: "command", error: { kind } },
    }),
  never: () => new Promise(() => undefined),
  sources: (overrides) => ({ ...pendingScreenSources, ...overrides }),
  connection: (state) => decode({ state }, setup.connectionSchema),
};

export const recorded = () =>
  Object.fromEntries(
    Object.entries(responses)
      .filter(([name]) => name !== "ipcErrors")
      .map(([name, value]) => [name, () => structuredClone(value)]),
  );

const replace = (rows, key, row) =>
  rows.map((item) => (item[key] === row[key] ? row : item));

export const makeSettingsClient = (overrides = {}) => {
  const data = structuredClone(settingsData);
  return {
    checkTossConnection: () => fx.connection("connected"),
    submitTossKeys: () => fx.connection("connected"),
    publicIp: () => decode(data.publicIp, setup.publicIpSchema),
    copyText: () => decode({ status: "copied" }, setup.copiedSchema),
    accountSelection: () => decode(data.accounts, setup.accountsSchema),
    selectAccount: (id) =>
      decode({ ...data.accounts, selected: id }, setup.accountsSchema),
    backupNow: () => decode({ status: "backed-up" }, setup.backupSchema),
    exportData: () => decode({ status: "exported" }, setup.exportSchema),
    deleteAllData: () => decode({ status: "deleted" }, setup.deleteSchema),
    aiSettings: () => decode(data.ai, ai.aiSettingsSchema),
    setTaskModel: (row) =>
      decode(
        { ...data.ai, tasks: replace(data.ai.tasks, "task", row) },
        ai.aiSettingsSchema,
      ),
    setBudgetPercent: (value) =>
      decode({ ...data.ai, budget_percent: value }, ai.aiSettingsSchema),
    setClaudeConfigDir: (value) =>
      decode({ ...data.ai, claude_config_dir: value }, ai.aiSettingsSchema),
    scheduledJobs: () => decode(data.jobs, ai.jobsSchema),
    setScheduledJob: (row) =>
      decode({ jobs: replace(data.jobs.jobs, "job", row) }, ai.jobsSchema),
    newsSettings: () => decode(data.news, news.newsSettingsSchema),
    setNewsSource: (row) =>
      decode(
        { ...data.news, sources: replace(data.news.sources, "source", row) },
        news.newsSettingsSchema,
      ),
    setOptionalKey: (key) =>
      decode(
        { ...data.news, keys: { ...data.news.keys, [key]: "set" } },
        news.newsSettingsSchema,
      ),
    setSecContact: (value) =>
      decode({ ...data.news, sec_contact: value }, news.newsSettingsSchema),
    setWakeCriteria: (value) =>
      decode({ ...data.news, wake: value }, news.newsSettingsSchema),
    setAliases: (value) =>
      decode({ ...data.news, aliases: value }, news.newsSettingsSchema),
    principles: () => decode(data.principles, setup.principlesSchema),
    setPrinciples: (value) => decode(value, setup.principlesSchema),
    ...overrides,
  };
};
