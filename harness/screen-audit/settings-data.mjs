export const settingsData = {
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
