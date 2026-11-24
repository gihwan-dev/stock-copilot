export type AppRoutePath =
  "/" | "/transactions" | "/settings" | "/settings/status";

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
} as const;
