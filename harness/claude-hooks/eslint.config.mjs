import scripts, {
  SCRIPT_FILES,
} from "../../apps/desktop/eslint.scripts.config.mjs";

export const HOOK_FILES = [".claude/hooks/**/*.mjs"];

export default scripts.map((block) =>
  block.files?.some((pattern) => SCRIPT_FILES.includes(pattern))
    ? { ...block, files: HOOK_FILES }
    : block,
);
