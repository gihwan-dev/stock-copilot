export const responses = {
  appVersion: { version: "0.1.0" },
  ipcErrors: [
    { kind: "user-fixable" },
    { kind: "retryable" },
    { kind: "bug" },
    { kind: "no-data" },
  ],
  openExternal: { status: "opened" },
} as const;
