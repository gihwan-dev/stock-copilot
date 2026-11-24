export const toolNames = [
  "get_portfolio",
  "get_allocation",
  "get_transactions",
  "get_performance",
  "get_quote",
  "get_candles",
  "get_market_overview",
  "get_rankings",
  "get_sectors",
  "get_investor_flow",
  "search_news",
  "get_disclosures",
  "read_article",
  "get_thesis",
  "update_thesis",
  "get_profile",
  "get_estimate_task",
  "submit_estimate",
  "get_scorecard",
  "request_web_research",
] as const;

export const toolTopics = {
  get_portfolio: "portfolio",
  get_allocation: "allocation",
  get_transactions: "transactions",
  get_performance: "performance",
  get_quote: "quote",
  get_candles: "candles",
  get_market_overview: "market",
  get_rankings: "rankings",
  get_sectors: "rankings",
  get_investor_flow: "investor-flow",
  search_news: "news",
  get_disclosures: "disclosures",
  read_article: "article",
  get_thesis: "thesis",
  update_thesis: "thesis",
  get_profile: "profile",
  get_estimate_task: "estimate",
  submit_estimate: "estimate",
  get_scorecard: "scorecard",
  request_web_research: "web-research",
} as const;

export const toolStatuses = {
  Running: "running",
  Succeeded: "succeeded",
  Failed: "failed",
} as const;

export const toolStatusesCodes = ["Running", "Succeeded", "Failed"] as const;

export const cardKinds = {
  Signal: "signal",
  Proposal: "proposal",
  Holding: "holding",
  Quote: "quote",
  Briefing: "briefing",
  Disclosure: "disclosure",
} as const;

export const cardKindsCodes = [
  "Signal",
  "Proposal",
  "Holding",
  "Quote",
  "Briefing",
  "Disclosure",
] as const;

export const sourceKinds = {
  Tool: "tool",
  Disclosure: "disclosure",
  News: "news",
  Web: "web",
} as const;

export const sourceKindsCodes = ["Tool", "Disclosure", "News", "Web"] as const;

export const failures = {
  IsolationRejected: "isolation",
  IsolationNotVerified: "isolation",
  McpUnavailable: "connection",
  ProviderUnavailable: "connection",
  InvalidResponse: "invalid-response",
  InvalidCredential: "credential",
  ForbiddenToolAttempt: "forbidden-tool",
  EmptyMessage: "empty-message",
  PromptTooLong: "too-long",
  StorageError: "storage",
  QueueBusy: "busy",
} as const;

export const failuresCodes = [
  "IsolationRejected",
  "IsolationNotVerified",
  "McpUnavailable",
  "ProviderUnavailable",
  "InvalidResponse",
  "InvalidCredential",
  "ForbiddenToolAttempt",
  "EmptyMessage",
  "PromptTooLong",
  "StorageError",
  "QueueBusy",
] as const;

export const turnStatuses = {
  InProgress: "in-progress",
  Completed: "completed",
  Cancelled: "cancelled",
  Interrupted: "interrupted",
  RateLimited: "rate-limited",
  Failed: "failed",
} as const;

export const turnStatusesCodes = [
  "InProgress",
  "Completed",
  "Cancelled",
  "Interrupted",
  "RateLimited",
  "Failed",
] as const;
