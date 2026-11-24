export type AppRoutePath =
  "/" | "/transactions" | "/briefings" | "/settings" | "/settings/status";

export const queryKeys = {
  appVersion: ["app-version"] as const,
  chatHistory: ["chat-history"] as const,
  chatResume: ["chat-resume"] as const,
  portfolio: ["portfolio"] as const,
  allocation: ["allocation"] as const,
  transactions: ["transactions"] as const,
  settings: ["settings"] as const,
  selfCheck: ["self-check"] as const,
  status: ["status"] as const,
  signals: ["signals"] as const,
  scorecard: ["scorecard"] as const,
  instruments: ["instruments"] as const,
  briefings: ["briefings"] as const,
  thesis: ["thesis"] as const,
} as const;

export const screenKeys = {
  portfolio: [...queryKeys.portfolio, "view"] as const,
  allocation: [...queryKeys.allocation, "view"] as const,
  transactions: [...queryKeys.transactions, "view"] as const,
  selfCheck: [...queryKeys.selfCheck, "view"] as const,
  status: [...queryKeys.status, "view"] as const,
  signals: [...queryKeys.signals, "view"] as const,
  scorecard: [...queryKeys.scorecard, "view"] as const,
  instruments: [...queryKeys.instruments, "view"] as const,
  briefings: [...queryKeys.briefings, "view"] as const,
} as const;

export const settingsKeys = {
  ai: [...queryKeys.settings, "ai"] as const,
  scheduledJobs: [...queryKeys.settings, "scheduled-jobs"] as const,
  news: [...queryKeys.settings, "news"] as const,
  principles: [...queryKeys.settings, "principles"] as const,
} as const;

export type AppQueryKey = (typeof queryKeys)[keyof typeof queryKeys];
