import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    include: ["src/**/*.test.ts", "src/**/*.test.tsx"],
    allowOnly: false,
    passWithNoTests: true,
    expect: { requireAssertions: true },
    unstubGlobals: true,
  },
});
