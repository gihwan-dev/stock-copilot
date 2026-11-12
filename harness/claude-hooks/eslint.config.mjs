import scripts, {
  SCRIPT_FILES,
} from "../../apps/desktop/eslint.scripts.config.mjs";
import { local } from "../../apps/desktop/eslint/local-plugin.mjs";

export const HOOK_FILES = [".claude/hooks/**/*.mjs"];

const RESTRICTED = "@eslint-community/eslint-comments/no-restricted-disable";

const hookBlocks = scripts.map((block) =>
  block.files?.some((pattern) => SCRIPT_FILES.includes(pattern))
    ? { ...block, files: HOOK_FILES }
    : block,
);

const restricted = hookBlocks
  .map((block) => block.rules?.[RESTRICTED])
  .find((setting) => setting !== undefined);

export default [
  ...hookBlocks,
  {
    files: HOOK_FILES,
    plugins: { local },
    rules: {
      "local/no-comments": "error",
      [RESTRICTED]: [...new Set([...restricted, "local/no-comments"])],
    },
  },
];
