import { createRequire } from "node:module";
import { join } from "node:path";
import { describe, it } from "node:test";
import { local } from "../eslint/local-plugin.mjs";
import { desktopRoot, repoRoot } from "./paths.mjs";

const requireFromDesktop = createRequire(join(desktopRoot, "package.json"));
const { RuleTester } = requireFromDesktop("eslint");
const tseslint = requireFromDesktop("typescript-eslint");

const harness = join(repoRoot, "harness", "eslint-violations");
process.chdir(harness);

RuleTester.describe = describe;
RuleTester.it = it;

const tester = new RuleTester({ languageOptions: { parser: tseslint.parser } });
const at = (path) => join(harness, path);
const denied = [{ messageId: "testingImport" }];

tester.run("local/ipc-testing-boundary", local.rules["ipc-testing-boundary"], {
  valid: [
    {
      code: 'import { a } from "./testing/fakes";',
      filename: at("src/shared/ipc/call.test.ts"),
    },
    {
      code: 'import { a } from "../shared/ipc/testing";',
      filename: at("src/app/App.test.tsx"),
    },
    {
      code: 'import { a } from "./mockBindings";',
      filename: at("src/shared/ipc/testing/fakes.ts"),
    },
    {
      code: 'import { a } from "@testing-library/react";',
      filename: at("src/app/App.tsx"),
    },
    {
      code: 'import { a } from "./call";',
      filename: at("src/shared/ipc/index.ts"),
    },
  ],
  invalid: [
    {
      code: 'import { a } from "./testing/fakes";',
      filename: at("src/shared/ipc/call.ts"),
      errors: denied,
    },
    {
      code: 'import type { A } from "./testing";',
      filename: at("src/shared/ipc/index.ts"),
      errors: denied,
    },
    {
      code: 'export { a } from "./testing/fakes";',
      filename: at("src/shared/ipc/index.ts"),
      errors: denied,
    },
    {
      code: 'export * from "../ipc/testing/fakes";',
      filename: at("src/shared/ipc/index.ts"),
      errors: denied,
    },
    {
      code: 'export const f = () => import("./testing/fakes");',
      filename: at("src/shared/ipc/call.ts"),
      errors: denied,
    },
    {
      code: 'export type T = typeof import("./testing/fakes");',
      filename: at("src/shared/ipc/call.ts"),
      errors: denied,
    },
    {
      code: 'import { a } from "./testing/fakes";',
      filename: at("src/shared/ipc/testing.ts"),
      errors: denied,
    },
  ],
});
