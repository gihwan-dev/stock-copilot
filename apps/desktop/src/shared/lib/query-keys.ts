export type AppRoutePath =
  "/" | "/transactions" | "/settings" | "/settings/status";

export const queryKeys = {
  appVersion: ["app-version"] as const,
  portfolio: ["portfolio"] as const,
  allocation: ["allocation"] as const,
  transactions: ["transactions"] as const,
  settings: ["settings"] as const,
  selfCheck: ["self-check"] as const,
  status: ["status"] as const,
} as const;

export const screenKeys = {
  portfolio: [...queryKeys.portfolio, "view"] as const,
  allocation: [...queryKeys.allocation, "view"] as const,
  transactions: [...queryKeys.transactions, "view"] as const,
  selfCheck: [...queryKeys.selfCheck, "view"] as const,
  status: [...queryKeys.status, "view"] as const,
} as const;

export type AppQueryKey = (typeof queryKeys)[keyof typeof queryKeys];
