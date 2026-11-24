import { join } from "node:path";
import { desktopRoot } from "./paths.mjs";

const src = (path) => JSON.stringify(join(desktopRoot, "src", path));

export const fakeData = {
  publicIp: { address: "203.0.113.7" },
  accounts: {
    selected: "acct-1",
    accounts: [
      { id: "acct-1", number: "123-45-0001" },
      { id: "acct-2", number: "123-45-0002" },
    ],
  },
  ai: {
    tasks: [
      { task: "chat", model: "opus", effort: "medium" },
      { task: "estimate", model: "opus", effort: "high" },
      { task: "deep-research", model: "opus", effort: "high" },
      { task: "briefing", model: "sonnet", effort: "medium" },
      { task: "news-summary", model: "haiku", effort: "low" },
      { task: "collection", model: "haiku", effort: "low" },
    ],
    budget_percent: "70",
    usage_percent: "42.5",
    claude_config_dir: null,
  },
  jobs: {
    jobs: [
      { job: "morning-briefing", state: "on", time: "08:20" },
      { job: "market-watch", state: "on", time: null },
      { job: "closing-review", state: "on", time: "15:50" },
      { job: "us-premarket", state: "on", time: null },
      { job: "estimate-rotation", state: "on", time: null },
      { job: "discovery", state: "on", time: "16:30" },
      { job: "weekly-review", state: "on", time: "09:00" },
    ],
  },
  news: {
    sources: [
      {
        source: "KindDisclosure",
        state: "on",
        body_reading: "off",
        interval_minutes: "2",
      },
      {
        source: "Hankyung",
        state: "on",
        body_reading: "off",
        interval_minutes: "30",
      },
      {
        source: "Yonhap",
        state: "on",
        body_reading: "locked",
        interval_minutes: "10",
      },
      {
        source: "SecEdgar",
        state: "off",
        body_reading: "off",
        interval_minutes: "60",
      },
    ],
    keys: { dart: "unset", fred: "set" },
    sec_contact: null,
    wake: { daily_move_percent: "5", news_per_hour: "5" },
    aliases: [{ alias: "SAMSUNG", ticker: "005930" }],
  },
  principles: {
    text: "",
    cash_min_percent: "10",
    concentration_min_count: "5",
  },
};

export const fakeSource = `
  import { settleWithSchema } from ${src("shared/ipc/schema.ts")};
  import * as setup from ${src("shared/ipc/setup-schema.ts")};
  import * as ai from ${src("shared/ipc/ai-schema.ts")};
  import * as news from ${src("shared/ipc/news-schema.ts")};
  const data = ${JSON.stringify(fakeData)};
  const decode = (value, schema) =>
    Promise.resolve(settleWithSchema({ status: "ok", data: value }, schema));
  const calls = [];
  const record = (name, args) => calls.push({ name, args: JSON.stringify(args) });
  const replace = (rows, key, row) => rows.map((r) => (r[key] === row[key] ? row : r));
  const wrap = (fns) =>
    Object.fromEntries(
      Object.entries(fns).map(([name, fn]) => [
        name,
        (...args) => {
          record(name, args);
          return fn(...args);
        },
      ]),
    );
  window.fakeCalls = calls;
  window.connectionReply = (state) => decode({ state }, setup.connectionSchema);
  window.failedReply = (kind) =>
    Promise.resolve(settleWithSchema({ status: "error", error: { kind } }, setup.connectionSchema));
  window.sequence = (states) => {
    let index = 0;
    return () => {
      const state = states[Math.min(index, states.length - 1)];
      index += 1;
      return window.connectionReply(state);
    };
  };
  window.makeFake = (overrides = {}) =>
    wrap({
      checkTossConnection: () => window.connectionReply("connected"),
      submitTossKeys: () => window.connectionReply("connected"),
      publicIp: () => decode(data.publicIp, setup.publicIpSchema),
      copyText: () => decode({ status: "copied" }, setup.copiedSchema),
      accountSelection: () => decode(data.accounts, setup.accountsSchema),
      selectAccount: (id) => decode({ ...data.accounts, selected: id }, setup.accountsSchema),
      backupNow: () => decode({ status: "backed-up" }, setup.backupSchema),
      exportData: () => decode({ status: "exported" }, setup.exportSchema),
      deleteAllData: () => decode({ status: "deleted" }, setup.deleteSchema),
      aiSettings: () => decode(data.ai, ai.aiSettingsSchema),
      setTaskModel: (s) => decode({ ...data.ai, tasks: replace(data.ai.tasks, "task", s) }, ai.aiSettingsSchema),
      setBudgetPercent: (p) => decode({ ...data.ai, budget_percent: p }, ai.aiSettingsSchema),
      setClaudeConfigDir: (p) => decode({ ...data.ai, claude_config_dir: p }, ai.aiSettingsSchema),
      scheduledJobs: () => decode(data.jobs, ai.jobsSchema),
      setScheduledJob: (s) => decode({ jobs: replace(data.jobs.jobs, "job", s) }, ai.jobsSchema),
      newsSettings: () => decode(data.news, news.newsSettingsSchema),
      setNewsSource: (s) => decode({ ...data.news, sources: replace(data.news.sources, "source", s) }, news.newsSettingsSchema),
      setOptionalKey: (k) => decode({ ...data.news, keys: { ...data.news.keys, [k]: "set" } }, news.newsSettingsSchema),
      setSecContact: (c) => decode({ ...data.news, sec_contact: c }, news.newsSettingsSchema),
      setWakeCriteria: (w) => decode({ ...data.news, wake: w }, news.newsSettingsSchema),
      setAliases: (a) => decode({ ...data.news, aliases: a }, news.newsSettingsSchema),
      principles: () => decode(data.principles, setup.principlesSchema),
      setPrinciples: (p) => decode(p, setup.principlesSchema),
      ...overrides,
    });
`;

export const selectByLabel = (w, label) => {
  const tag = Array.from(w.document.body.querySelectorAll("label")).find(
    (node) => node.textContent === label && node.htmlFor !== "",
  );
  return tag === undefined ? null : w.document.getElementById(tag.htmlFor);
};

export const choose = (w, label, value) => {
  const select = selectByLabel(w, label);
  if (select === null) throw new Error(`no select ${label}`);
  w.fireEvent.change(select, { target: { value } });
};

export const okReply = (w, value) => w.Promise.resolve({ kind: "ok", value });
